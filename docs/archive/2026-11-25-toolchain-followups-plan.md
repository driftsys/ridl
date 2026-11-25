# Toolchain Follow-ups Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close the gaps #1476 left: a refused terminal command runs a
compatible installed release, `ridl mcp` does the same before it serves, an
opt-in terminal install, `ridl toolchain uninstall`, minisign-signed release
archives, a Restricted Mode install prompt in VS Code, and the six smaller
review items of driftsys/ridl#1477.

**Architecture:** `crates/ridl` keeps all toolchain code. The library
(`ridl_cli::toolchain`) gains root resolution from the process, uninstall,
signature verification and multi-pin selection; a new binary module
`crates/ridl/src/handoff.rs` is called at each enforcing refusal site in the
`ridl` binary before anything is rendered, and replaces the process (Unix) or
waits on a child in a Job Object (Windows). The release workflow signs each
`ridl-<target>.tar.gz` with minisign; the CLI embeds the public key. The VS Code
extension declares limited untrusted support and, in Restricted Mode, runs only
`ridl toolchain which --json` with its bundled binary.

**Tech Stack:** Rust 1.98.1, clap, `minisign-verify` 0.3, `windows-sys` 0.61
(Windows only), ureq 3, sha2, tar, flate2; the `minisign` 0.10 crate as a
test-only signer; Bash, PowerShell; GitHub Actions with the Ubuntu `minisign`
package; TypeScript, Node's test runner.

**Spec:**
[Toolchain follow-ups design](2026-11-25-toolchain-followups-design.md),
including its "Builder check resolutions" section. Read it with this plan. The
#1476 records it extends:
[ADR-0036](../decisions/ADR-0036-workspace-toolchain-pins.md),
[the toolchain directory record](../design/toolchain-directory.md),
[the release pin chapter](../book/release-pin.md) and
[the distribution technote](../technotes/toolchain-distribution.md).

## Global Constraints

- Read `AGENTS.md` before editing. Prose (comments, docs, commit messages) is
  plain literal English with no idioms. Shipped docs, code and rustdoc name no
  story ids, plan names or task numbers (`just story-id-check`); link
  `driftsys/ridl#1477` where a gap is real.
- Only MANI-026 hands off. MANI-024 and MANI-025 never do. A run hands off only
  when every release refusal in it is MANI-026.
- Selection for a hand-off: the highest complete, undamaged installed release
  compatible with **every** MANI-026 pin of the run. The running binary is never
  a candidate. No such release: no hand-off; the refusal prints as today, exit
  2.
- Guard variable `RIDL_TOOLCHAIN_HANDOFF`, value the parent's running release
  (`ridlc::running_release().to_string()`). A process where the variable is
  present (any value, including empty) never hands off and never installs.
- Automatic install variable `RIDL_TOOLCHAIN_AUTO_INSTALL`: on only when the
  value is exactly `1`; only when the guard is absent; only when the run has
  exactly one distinct MANI-026 pin; never from `ridl mcp` or `ridl lsp`; no
  `ridl.toml` key enables it.
- The hand-off prints nothing on success; output and exit status are the
  selected release's. An `exec` failure prints
  `error: could not run ridl <version> at <path>: <io error>` and exits 2.
- `ridlc` keeps its refusals; hand-off sites are only in `crates/ridl/src/`.
  `ridl toolchain *`, `init`, `new`, `lock merge`, snapshot-only `diff`,
  standalone descriptors and `ridl lsp` never hand off.
- Pins are re-parsed from the diagnostic's file text with
  `ridl_core::release::parse_release_pin` (already public). No `Diagnostic`
  field is added and no message text is parsed. No `ridl-core` API change.
- Signing: minisign prehashed (`ED`) signatures only; legacy (`Ed`) signatures
  are refused (`PublicKey::verify(.., allow_legacy = false)`). Trusted comment
  is exactly `ridl <version> <target>`, `<version>` in canonical
  `MAJOR.MINOR.PATCH` display. `.minisig` download bound: 4096 bytes. Order:
  archive download, `.sha256` download, checksum check, `.minisig` download,
  signature check, extraction. `ridlc` archives are not signed.
- The production public key constant `RELEASE_PUBLIC_KEY` in
  `crates/ridl/src/toolchain/mod.rs` holds a placeholder until the owner
  supplies the key. The placeholder value contains the fixed marker
  `RIDL-RELEASE-KEY-PLACEHOLDER`; the release workflow's signing step fails
  while `crates/ridl/src/toolchain/mod.rs` contains that marker. No CI test
  enforces the key. **The branch is not merged while it holds the placeholder**
  (merge checklist). Never generate a production key pair. Test key pairs are
  generated in memory at test time by the `minisign` crate; no secret key file
  is committed.
- No `FIRST_SIGNED_RELEASE` constant: this plan assumes the change lands before
  0.23.0 is tagged (design, builder resolution 6). If 0.23.0 is tagged first,
  stop and ask before merging.
- New direct dependencies: `minisign-verify = "0.3"` in `crates/ridl`
  `[dependencies]`;
  `windows-sys = { version = "0.61", features = ["Win32_Foundation", "Win32_System_JobObjects", "Win32_System_Threading", "Win32_System_Console"] }`
  under `[target.'cfg(windows)'.dependencies]`; and the dev-dependency
  `minisign = "0.10"` in `crates/ridl` only. `minisign` is dev-only: it signs
  test fixtures and never reaches the `ridl` binary, whose only signature
  dependency is `minisign-verify`. New `Cargo.lock` entries are accepted for
  `minisign-verify`, `minisign` and the packages `minisign` pulls in
  (`ct-codecs`, `scrypt` and their dependencies, `rpassword`); no other new
  `[[package]]`. After adding a dependency, run
  `cargo check -p ridl-cli
  --tests` once without `--locked`, then review
  `git diff Cargo.lock`.
- Windows: no Windows CI job is added by this change. Platform-independent
  hand-off logic (pin collection, selection, guard and auto-install decisions,
  child argument and environment assembly) lives in functions tested on every
  platform; the `cfg(windows)` code is only the Job Object, the console handler
  and the wait.
- `RIDL_TOOLCHAIN_DIR`: a nonempty relative value is an error, exit 2, in the
  CLI and in both scripts; empty counts as unset. Other variables
  (`XDG_DATA_HOME`, `LOCALAPPDATA`) keep today's behaviour.
- Exit codes follow ADR-0010: `toolchain uninstall` is 0 removed, 1 not
  installed, 2 usage, running-release refusal or I/O error.
- Frozen surfaces extend by addition only. Only `freeze/cli.txt` changes (the
  `uninstall` subcommand); environment variables and release assets are not in a
  dump. Regenerate with `cargo xtask freeze update`, keep only added lines.
- Debug-only test seams follow the `RIDL_TEST_RUNNING_RELEASE` precedent: read
  only under `cfg(debug_assertions)`, never in a release build.
- Each test cycle compiles before its red run and fails on an assertion. For a
  new interface, first add the minimum compiling declaration with an inert
  result in the named file. A missing symbol, a `todo!()` panic or a missing
  fixture is not the required red result. For each test, apply the named
  mutation, see the test fail, restore, rerun.
- Implementers run focused tests only: `cargo test --locked -p <crate>`,
  `npm test` in `editors/vscode`, `just install-check`, `just freeze-check`,
  `just link-check`, `just doc-path-check`. CI runs the full gates. Tests that
  spawn a fake installed `ridl` are `#[cfg(unix)]`.
- Tests that produce a release refusal set `RIDL_TOOLCHAIN_DIR` to an empty
  temporary directory and remove `RIDL_TOOLCHAIN_HANDOFF` and
  `RIDL_TOOLCHAIN_AUTO_INSTALL` from the child environment, so that a
  developer's real toolchain directory cannot change their result.
- Commits: Conventional Commits with a scope from `.git-std.toml`. End each
  commit message with the attribution line the session gives. `docs/wip/` stays
  until the branch is gardened.

## Review Focus

1. A run whose refusals mix MANI-024 and MANI-026 (`fmt` over two roots) with a
   compatible release installed must not hand off: both refusals print and the
   exit is 2 — Task 3, `handoff_is_skipped_when_any_refusal_is_not_mani_026`.
2. Arguments containing spaces and a toolchain root containing spaces reach the
   child byte for byte — Task 3,
   `handoff_preserves_arguments_with_spaces_and_a_root_with_spaces`.
3. `ridl toolchain uninstall` on a version entry that is a symbolic link to a
   directory outside the versions directory removes the link and leaves the
   target untouched — Task 1,
   `toolchain_uninstall_removes_a_symlink_entry_without_following_it`.
4. A legacy (non-prehashed) signature and a `.minisig` larger than 4096 bytes
   are refused, the latter with nothing published — Task 2,
   `signature_refuses_a_legacy_signature` and
   `toolchain_install_requires_a_valid_signature` (case `oversize`).
5. Two RIDL roots in Restricted Mode requiring the same release show one prompt,
   and pressing Install twice runs one install — Task 6,
   `restricted_prompt_is_shown_once_per_release_across_roots`.

---

## File Structure

- `crates/ridl/src/toolchain/paths.rs` — root resolution, now also from the
  process (`process_toolchain_root`), refusing a relative override.
- `crates/ridl/src/toolchain/uninstall.rs` (new) — removal under the version
  lock.
- `crates/ridl/src/toolchain/install.rs` — gains `.minisig` download and
  `verify_signature`; its stale-stage loop becomes a shared helper.
- `crates/ridl/src/toolchain/selection.rs` — gains `select_installed`.
- `crates/ridl/src/toolchain/mod.rs` — `RELEASE_PUBLIC_KEY`, `BUILD_TARGET`,
  re-exports.
- `crates/ridl/src/handoff.rs` (new, binary module) — pins, selection, guard,
  automatic install, process replacement.
- `crates/ridl/src/main.rs`, `lock.rs`, `property.rs`, `vendor.rs` — one hook
  call per refusal site; `uninstall` subcommand; `run_describe` ordering;
  `run_mcp` hand-off.
- `crates/ridlc/src/diff_side.rs` — refusal before input I/O errors.
- `crates/ridl/tests/handoff.rs` (new), `release_signing.rs` (new),
  `toolchain.rs`, `toolchain_http.rs`, `release_pin.rs`,
  `toolchain_support/mod.rs`.
- `install.sh`, `install.ps1`, `justfile` (`install-check` only).
- `.github/workflows/vscode-release.yaml` (`release` job only).
- `editors/vscode/src/restrictedMode.ts` (new) and its test;
  `binaryResolution.ts`, `toolchainSelection.ts`, `extension.ts`,
  `package.json`, `README.md`.

Test assertions below are assertion clauses, not test bodies. Bindings named in
a clause are values observed from the fixture the step describes.

### Task 1: Uninstall releases, refuse a relative root, and order refusals before input I/O errors

**Files:**

- Modify: `crates/ridl/src/toolchain/paths.rs`,
  `crates/ridl/src/toolchain/mod.rs`,
  `crates/ridl/src/toolchain/install.rs:178-191` (extract the stale-stage loop).
- Create: `crates/ridl/src/toolchain/uninstall.rs`.
- Modify: `crates/ridl/src/main.rs:394-489` (`ToolchainCommand`,
  `run_toolchain`), `crates/ridl/src/main.rs:640-655` (`run_describe`).
- Modify: `crates/ridlc/src/diff_side.rs:209-261` (`load_diff_side`).
- Test: `crates/ridl/tests/toolchain.rs`, `crates/ridl/tests/release_pin.rs`,
  `xtask/src/freeze/cli.rs` (`cli_dump_lists_toolchain_commands`).
- Docs: `docs/book/cli-reference.md` (toolchain parent transcript, new
  `ridl toolchain uninstall` section, exit-table row),
  `docs/decisions/ADR-0010-cli-conventions.md` (decision 1 table row and the
  evidence paragraph after "earned their rows on 2026-11-25"), `freeze/cli.txt`,
  `docs/design/toolchain-directory.md` (layout, new "Uninstall" section,
  regression evidence), `docs/book/release-pin.md` (relative override sentence,
  uninstall paragraph, the `describe` and snapshot `diff` ordering sentence).

**Interfaces:**

- Consumes:
  `ridl_core::release::{Release, parse_release, MIN_PINNABLE_RELEASE}`;
  `ridl_core::read_root_release`; existing `install::inspect`.
- Produces in `ridl_cli::toolchain`:
  - `pub fn process_toolchain_root() -> io::Result<PathBuf>` — collects
    `std::env::vars_os()` (UTF-8 keys), `HOME`, `std::env::consts::OS` and calls
    `toolchain_root`. `run_toolchain` uses it; Task 3 consumes it.
  - `toolchain_root` returns
    `io::Error::new(io::ErrorKind::InvalidInput, "RIDL_TOOLCHAIN_DIR must be an absolute path, not`<value>`")`
    for a nonempty value where `Path::is_absolute()` is false. Absolute values
    are returned unchanged.
  - `pub fn uninstall(root: &Path, release: Release, running_exe: &Path) -> io::Result<Option<PathBuf>>`
    — `Ok(None)` when `<root>/versions` or the entry is absent; `Ok(Some(p))`
    with `p = <canonical versions>/<release>` after removal.
  - `pub(super) fn remove_stale_stages(versions: &Path, release: Release) -> io::Result<()>`
    in `install.rs` — the existing loop of lines 178-191, unchanged in
    behaviour, called by `install` and `uninstall`.
- Produces in `main.rs`: `ToolchainCommand::Uninstall { version: String }` with
  doc comment `Remove an installed release directory, complete or damaged.` and
  argument doc `The exact MAJOR.MINOR.PATCH release to remove.`

- [ ] **Step 1: Write the failing relative-root tests.** In
      `tests/toolchain.rs`, replace the assertion in
      `toolchain_roots_follow_platform_precedence` that a relative value
      resolves against the current directory with: for each of `linux`, `macos`,
      `windows`, `toolchain_root(platform, &env, None)` with
      `RIDL_TOOLCHAIN_DIR = "relative tools"` is an error. Add
      `toolchain_cli_refuses_a_relative_root`:

  ```rust
  assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
  assert!(error.to_string().contains("RIDL_TOOLCHAIN_DIR must be an absolute path"));
  // `ridl toolchain list` run in `w` with RIDL_TOOLCHAIN_DIR="relative tools"
  assert_eq!(list.status.code(), Some(2));
  assert!(list.stdout.is_empty());
  assert!(String::from_utf8_lossy(&list.stderr).contains("RIDL_TOOLCHAIN_DIR must be an absolute path"));
  assert!(!w.path().join("relative tools").exists());
  ```

  Mutation killed: keep `std::path::absolute(path)` for the override.

- [ ] **Step 2: Run them to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test toolchain toolchain_roots` and
      `... toolchain_cli_refuses_a_relative_root`. Expected: FAIL on the
      `is_err`/exit-code assertions.

- [ ] **Step 3: Implement the refusal in `toolchain_root` and add
      `process_toolchain_root`.** Replace the inline environment collection in
      `run_toolchain` with `process_toolchain_root()`.

- [ ] **Step 4: Run them to verify they pass.** Same commands. Expected: PASS.

- [ ] **Step 5: Write the failing uninstall tests.** First add the inert
      `uninstall` returning `Ok(None)` and the `Uninstall` variant whose
      `run_toolchain` arm prints nothing and exits 1, so the tests compile. Then
      add to `tests/toolchain.rs`:

  - `toolchain_uninstall_removes_complete_and_damaged_entries`. Fixture:
    `installed(root, "0.23.0")`, `installed(root, "0.23.1")`, a damaged `0.23.2`
    (no `.complete`), a regular file named `0.23.3`, a stale stage directory
    `.tmp-0.23.0-12345` with a sentinel, and `.tmp-script-0.23.0-1` with a
    sentinel.

    ```rust
    assert_eq!(removed, Some(canonical_versions.join("0.23.0")));
    assert!(!canonical_versions.join("0.23.0").exists());
    assert_eq!(uninstall(root, release("0.23.2"), &running())?, Some(canonical_versions.join("0.23.2")));
    assert_eq!(uninstall(root, release("0.23.3"), &running())?, Some(canonical_versions.join("0.23.3")));
    assert_eq!(uninstall(root, release("0.23.4"), &running())?, None);
    assert_eq!(directory_bytes(&canonical_versions.join("0.23.1")), before_0_23_1);
    assert!(!canonical_versions.join(".tmp-0.23.0-12345").exists());
    assert!(canonical_versions.join(".tmp-script-0.23.0-1/sentinel").exists());
    assert!(names_starting_with(&canonical_versions, ".tmp-0.23.").is_empty());
    assert_eq!(uninstall(empty_root, release("0.23.0"), &running())?, None);
    assert!(!empty_root.join("versions").exists());
    ```

    Mutations killed: return `None` for a damaged entry; skip the stage removal
    after the rename; create `versions/` when absent; skip
    `remove_stale_stages`.

  - `toolchain_uninstall_removes_a_symlink_entry_without_following_it`
    (`#[cfg(unix)]`, Review Focus 3): `versions/0.23.0` is a symbolic link to an
    outside directory holding `sentinel`.

    ```rust
    assert_eq!(removed, Some(canonical_versions.join("0.23.0")));
    assert!(std::fs::symlink_metadata(canonical_versions.join("0.23.0")).is_err());
    assert_eq!(fs::read(outside.join("sentinel"))?, b"keep");
    ```

    Mutation killed: canonicalize the entry before the rename, or call
    `remove_dir_all` on the followed path.

  - `toolchain_uninstall_refuses_the_running_release`: copy
    `env!("CARGO_BIN_EXE_ridl")` to `versions/0.23.0/<binary_name()>` with an
    empty `.complete`, then run that copy as `toolchain uninstall 0.23.0` with
    `RIDL_TOOLCHAIN_DIR=root`; also call the library with
    `running_exe = that copy`.

    ```rust
    assert_eq!(cli.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&cli.stderr).contains("running"));
    assert_eq!(directory_bytes(&dir), before);
    assert!(uninstall(root, release("0.23.0"), &copy).is_err());
    ```

    Mutation killed: delete the running-executable check.

  - `toolchain_uninstall_waits_for_the_version_lock`: the parent holds
    `File::lock` on `versions/.0.23.0.lock`, spawns
    `ridl toolchain uninstall 0.23.0`, sleeps 200 ms.

    ```rust
    assert!(child.try_wait()?.is_none(), "uninstall did not wait for the lock");
    assert!(dir.exists());
    // after unlock and wait
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout, format!("{}\n", canonical_versions.join("0.23.0").display()));
    assert!(!dir.exists());
    ```

    Mutation killed: drop the `lock()` call.

  - `toolchain_uninstall_cli_streams_and_exit_codes`:

    ```rust
    // absent 0.23.4
    output(&absent, 1, "", true);
    assert!(stderr(&absent).contains("ridl 0.23.4 is not installed"));
    // "0.23", "0.22.0", "v0.23.0"
    for invalid in invalid_outputs { output(&invalid, 2, "", true); }
    ```

    Mutation killed: map `Ok(None)` to exit 0, or skip the floor check.

- [ ] **Step 6: Run them to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test toolchain toolchain_uninstall`.
      Expected: FAIL on the `Some(..)`, exit-code and `try_wait` assertions.

- [ ] **Step 7: Implement
      `uninstall(root: &Path, release: Release, running_exe: &Path) -> io::Result<Option<PathBuf>>`
      in `toolchain/uninstall.rs`.** Order: canonicalize `<root>/versions`
      (`NotFound` gives `Ok(None)`);
      `entry = versions.join(release.to_string())`; refuse with
      `io::Error::other("ridl <v> at <dir> is the running release; run another ridl to remove it")`
      when `running_exe.canonicalize()` starts with `entry.canonicalize()` (skip
      the check when the entry does not resolve); open and `lock()`
      `.<release>.lock` as `install` does; `remove_stale_stages`;
      `symlink_metadata(entry)` (`NotFound` gives `Ok(None)`); rename the entry
      to `.tmp-<release>-<pid>` without following it; remove the stage with
      `remove_dir_all` when its `symlink_metadata` is a real directory,
      otherwise `remove_file`; return `Some(entry)`. Implement the
      `run_toolchain` arm: same version validation as `Install` (exit 2 on a bad
      string or a release below 0.23.0), `Some(p)` prints `p` and newline on
      stdout with exit 0, `None` prints
      `ridl <v> is not installed in <canonical versions or root/versions>` on
      stderr with exit 1, errors exit 2.

- [ ] **Step 8: Run them to verify they pass.** Same command. Expected: PASS.

- [ ] **Step 9: Write the failing ordering tests.** In `tests/release_pin.rs`,
      using the existing `run` helper (pin `0.24.0`, running `0.23.2`):

  - `release_describe_refusal_precedes_a_missing_input`: in a pinned workspace,
    `describe missing.catalog.binfb` and `describe gone/missing.catalog.binfb`
    (no `gone/`). Control: the same two commands in an unpinned temporary
    directory.

    ```rust
    assert_refusal(&pinned, "MANI-026");
    assert!(!stderr(&pinned).contains("No such file"));
    assert_eq!(control.status.code(), Some(2));
    assert!(stderr(&control).starts_with("error: missing.catalog.binfb: "));
    ```

  - `release_diff_refusal_precedes_snapshot_input_errors` (`#[cfg(unix)]`):
    `snaps/` in a pinned workspace holds a dangling symbolic link
    `x.ir.json -> missing` (listing error); a second case holds a readable
    `a.ir.json` copied from a built snapshot plus `b.ir.json` with mode `000`
    (read error; skip the case when running as root). Run `diff snaps snaps`.
    Control: the same directories outside any workspace.

    ```rust
    assert_refusal(&listing, "MANI-026");
    assert_refusal(&reading, "MANI-026");
    assert_eq!(control_listing.status.code(), Some(2));
    assert!(stderr(&control_listing).starts_with("error: cannot read the snapshot "));
    assert_eq!(control_reading.status.code(), Some(2));
    assert!(stderr(&control_reading).contains("b.ir.json: "));
    ```

    The control messages are today's `DiffSideError` texts and must not change.
    Mutations killed: apply the refusal only to the listing error; reorder
    `metadata` before the preflight in `run_describe`.

  The existing
  `release_diff_snapshot_directory_with_stray_source_ignores_parent_pin` must
  stay green: a readable snapshot directory is still not refused.

- [ ] **Step 10: Run them to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test release_pin release_describe_refusal_precedes release_diff_refusal_precedes`.
      Expected: FAIL on `assert_refusal` (today the I/O error prints first).

- [ ] **Step 11: Implement the ordering.** In `run_describe`, when
      `std::fs::metadata(path)` fails, call `release_preflight_exit` with the
      nearest existing ancestor directory of `path` (falling back to `.`); if it
      returns a code, return it; otherwise print the existing message. In
      `crates/ridlc/src/diff_side.rs` add
      `fn refusal_before_io(entry: &Path, overlays: &[Overlay], error: DiffSideError) -> DiffSideError`:
      for `SnapshotDirectory`, `SnapshotMetadata`, `SnapshotRead` and
      `NestedRead`, return the `Err` of
      `source_release_preflight(entry, overlays)` when it is `Compile`, else
      `error`. Apply it to the errors of `snapshot_files`,
      `first_nested_snapshot_dir` and `load_snapshots` in the directory branch
      of `load_diff_side`.

- [ ] **Step 12: Run them to verify they pass.** Same command, then
      `cargo test --locked -p ridlc diff_side` and
      `cargo test --locked -p ridl-cli --test release_pin`. Expected: PASS.

- [ ] **Step 13: Write the failing surface tests, then the docs.** Extend
      `cli_dump_lists_toolchain_commands` with `"ridl toolchain uninstall"` and
      `"ridl toolchain uninstall arg 1 <VERSION> required"`. Run
      `cargo test --locked -p xtask cli_dump_lists_toolchain_commands` and
      `cargo test --locked -p ridl-cli --test cli_reference`. Expected: FAIL
      ("committed dump missing", missing transcript and exit-table row). Then
      run `cargo xtask freeze update` and keep only the added `uninstall` lines;
      add the transcripts and the row (`removed directory printed` /
      `release not installed` /
      `invalid or below-floor version, running
      release, lock or I/O failure`)
      to `cli-reference.md` and the matching row plus one evidence sentence
      naming the four `toolchain_uninstall_*` tests to ADR-0010. Write the
      directory record's "Uninstall" section (lock, stage name, symlink not
      followed, running release refused, no script serialization) and the
      release-pin chapter sentences. In the chapter, replace "A relative
      override is resolved against the current directory." with "`ridl` refuses
      a relative override, exit 2." and add: "`describe` and a `diff` directory
      input report a root release refusal before an I/O error of the input."

- [ ] **Step 14: Run the surface and doc checks.** Run
      `cargo test --locked -p xtask freeze::cli`,
      `cargo test --locked -p ridl-cli --test cli_reference`,
      `cargo test --locked -p ridl-cli --test toolchain`, `just freeze-check`,
      `just link-check`, `just doc-path-check`. Expected: PASS, no removed dump
      lines.

- [ ] **Step 15: Commit.**
      `feat(ridl): add toolchain uninstall and refuse a relative toolchain directory`

### Task 2: Verify minisign signatures of release archives

**Files:**

- Modify: `crates/ridl/Cargo.toml` (`minisign-verify = "0.3"`; dev-only
  `minisign = "0.10"`), `Cargo.lock`.
- Modify: `crates/ridl/src/toolchain/mod.rs`,
  `crates/ridl/src/toolchain/install.rs`, `crates/ridl/src/main.rs` (the
  `install` call in `run_toolchain`).
- Modify: `crates/ridl/tests/toolchain_support/mod.rs`, `tests/toolchain.rs`,
  `tests/toolchain_http.rs`.
- Create: `crates/ridl/tests/release_signing.rs`.
- Modify: `.github/workflows/vscode-release.yaml` (`release` job only).
- Docs: `THIRD-PARTY-NOTICES.txt` (a `minisign-verify 0.3.0` entry in the flate2
  format, MIT text copied from the crate's `LICENSE`),
  `docs/design/toolchain-directory.md` ("Targets and download source", "CLI
  installation transaction" step 4, regression evidence),
  `docs/book/release-pin.md` (install paragraph), `docs/book/cli-reference.md`
  (install exit row: "download, checksum, signature or archive error").

**Interfaces:**

- Consumes: `install` and its private helpers as they are today.
- Produces in `ridl_cli::toolchain`:
  - `pub const RELEASE_PUBLIC_KEY: &str = "RIDL-RELEASE-KEY-PLACEHOLDER: the owner replaces this with the release minisign public key before merge";`
    with a doc comment stating it is a placeholder that fails every download,
    that the release workflow refuses to sign while the marker is present, and
    that it must be replaced before merge.
  - `pub fn install(root: &Path, release: Release, target: &str, base_url: &str, public_key: &str) -> io::Result<PathBuf>`.
    Production passes `RELEASE_BASE_URL` and `RELEASE_PUBLIC_KEY`.
- Private in `install.rs`: `const SIGNATURE_LIMIT: u64 = 4 * 1024;` and
  `fn verify_signature(bytes: &[u8], minisig: &[u8], public_key: &str, expected_comment: &str) -> io::Result<()>`.
- Produces in test support (`toolchain_support`), used by Tasks 3 and 4:
  - `pub fn test_public_key() -> String` and
    `pub fn other_test_public_key() -> String` — base64 public keys of two key
    pairs generated once per test process with
    `minisign::KeyPair::generate_unencrypted_keypair()` and held in
    `std::sync::OnceLock`s.
  - `pub fn sign(bytes: &[u8], trusted_comment: &str) -> String` — a prehashed
    signature from the first test key, through
    `minisign::sign(Some(&pk), &sk,
    bytes, Some(trusted_comment), Some("signature from a ridl test key"))`
    and `SignatureBox::into_string()`.
  - `pub fn fixture_for(base: &Path, version: &str, target: &str, entries: &[(&str, u8, &[u8])]) -> PathBuf`
    writing `editor-v<version>/ridl-<target>.tar.gz`, `.sha256` and `.minisig`
    (comment `ridl <version> <target>`); `fixture(base, entries)` keeps its
    signature and calls it with `"0.23.0"` and `TARGET`.
  - `pub fn write_asset(dir: &Path, bytes: &[u8])` now also signs;
    `pub fn write_unsigned_asset(dir: &Path, bytes: &[u8])` writes only the
    archive and the checksum.

- [ ] **Step 1: Add the test signer and its self-test.** Add the `minisign`
      dev-dependency and the support functions above. A comment in support
      states that the key pairs are generated per test process and never sign a
      release. Add `test_signer_produces_minisign_signatures` to
      `tests/toolchain.rs`:

  ```rust
  assert_ne!(test_public_key(), other_test_public_key());
  let key = minisign_verify::PublicKey::from_base64(&test_public_key()).unwrap();
  let sig = minisign_verify::Signature::decode(&sign(b"payload", "ridl 0.23.0 t")).unwrap();
  assert!(key.verify(b"payload", &sig, false).is_ok());
  assert_eq!(sig.trusted_comment(), "ridl 0.23.0 t");
  ```

  This is support code, so it is green once written; the next steps are the red
  cycles.

- [ ] **Step 2: Thread the key parameter with an inert check.** Change `install`
      to take `public_key: &str`, ignore it, pass `&support::test_public_key()`
      at every test call site in `tests/toolchain.rs` and
      `tests/toolchain_http.rs`, and `RELEASE_PUBLIC_KEY` in `run_toolchain`.
      Make `write_asset` sign. Update
      `toolchain_install_rechecks_the_requested_target_binary` to write its
      own-target `.minisig` with `sign(bytes, "ridl 0.23.0 <target>")`. Run
      `cargo test --locked -p ridl-cli --test toolchain`. Expected: PASS
      (nothing verifies yet).

- [ ] **Step 3: Write the failing signature tests.** In `tests/toolchain.rs`:

  - `toolchain_install_requires_a_valid_signature`, one fresh root per case,
    assets from `fixture`, then the case's change:

    | case           | change                                                              | error contains         |
    | -------------- | ------------------------------------------------------------------- | ---------------------- |
    | `wrong-key`    | install with `other_test_public_key()`                              | `signature`            |
    | `old-release`  | `.minisig` = `sign(archive, "ridl 0.22.9 <TARGET>")`                | `ridl 0.23.0 <TARGET>` |
    | `other-target` | `.minisig` = `sign(archive, "ridl 0.23.0 x86_64-pc-windows-msvc")`  | `ridl 0.23.0 <TARGET>` |
    | `prefix`       | `.minisig` = `sign(archive, "ridl 0.23.0 <TARGET> extra")`          | `ridl 0.23.0 <TARGET>` |
    | `missing`      | remove `.minisig`                                                   | `.minisig`             |
    | `tampered`     | `write_unsigned_asset(dir, other_archive_bytes)` (checksum matches) | `signature`            |
    | `oversize`     | `.minisig` of 4097 bytes                                            | `exceeds 4096`         |

    ```rust
    assert!(error.to_string().contains(expected), "{case}: {error}");
    assert!(!root.join("versions/0.23.0").exists(), "{case}");
    support::assert_no_stage(root);
    ```

    Mutations killed: skip the `verify_signature` call; compare the comment with
    `starts_with`; download `.minisig` with `ARCHIVE_LIMIT`.

  - `toolchain_install_refuses_an_undecodable_public_key_before_download`:
    `install(root, 0.23.0, TARGET, "file:///nonexistent-origin", "PLACEHOLDER")`.

    ```rust
    assert!(error.to_string().contains("release public key"));
    assert!(!error.to_string().contains("nonexistent-origin"));
    ```

    Mutation killed: decode the key after the downloads.

  - In `install.rs` unit tests, `signature_verifies_a_real_minisign_signature`:
    the public key, signature and message `b"test"` from the `minisign-verify`
    0.3.0 crate documentation, with
    `expected_comment = "timestamp:1633700835\tfile:test\tprehashed"`, is `Ok`;
    the same with expected comment `"x"` is `Err`. This pins compatibility with
    signatures made by the `minisign` tool.

  - In `install.rs` unit tests, `signature_refuses_a_legacy_signature` (Review
    Focus 4): the legacy (`Ed`) vector of the `minisign-verify` 0.3.0 crate's
    own `verify` test (public key
    `RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3`, message
    `b"test"`, trusted comment `timestamp:1555779966\tfile:test`), which that
    crate verifies only with `allow_legacy = true`, gives `Err` from
    `verify_signature` with its own trusted comment as `expected_comment`.
    Mutation killed: pass `allow_legacy = true`. (The `minisign` crate signs
    only prehashed, so the legacy case uses this published vector.)

  No test checks the production key; the release workflow does (Step 8).

- [ ] **Step 4: Run them to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test toolchain toolchain_install_re`
      and `cargo test --locked -p ridl-cli --lib signature_`. Expected: FAIL on
      `is_err` / message assertions; the real-signature test fails on its `Err`
      half while `verify_signature` is inert.

- [ ] **Step 5: Implement signature verification in `install.rs`.** Decode the
      public key right after the second `existing` check (an installed release
      is reused without decoding); error text
      `invalid release public key: <reason>`. After `verify` of the checksum,
      download `<url>.minisig` with `SIGNATURE_LIMIT`, then call
      `verify_signature(&bytes, &minisig, public_key, &format!("ridl {release} {target}"))`
      before `extract`. `verify_signature` decodes the UTF-8 text with
      `Signature::decode`, calls `PublicKey::verify(bytes, &sig, false)`, then
      requires `sig.trusted_comment() == expected_comment`. Errors are
      `InvalidData` with
      `invalid release signature for ridl-<target>.tar.gz: <reason>` or
      `release signature for ridl-<target>.tar.gz is for`<got>`, expected`<expected>``.

- [ ] **Step 6: Run them to verify they pass.** Run
      `cargo test --locked -p ridl-cli --test toolchain` and
      `cargo test --locked -p ridl-cli --lib`. Expected: PASS.

- [ ] **Step 7: Update the HTTP fixture test.** In the server loop of
      `toolchain_ureq_path_verifies_and_installs`, serve five requests and
      assert
      `paths == [archive, archive.sha256, archive.minisig, archive, archive.sha256]`
      (the bad-checksum second install stops before the signature). Run
      `cargo test --locked -p ridl-cli --test toolchain_http -- --ignored`
      outside a sandbox that forbids TCP. Expected before step 5: FAIL on the
      path list; after: PASS.

- [ ] **Step 8: Write the failing workflow tests.** Create
      `tests/release_signing.rs`. It extracts the `run: |` script of the step
      named `sign the ridl archives` from `vscode-release.yaml` the way
      `formatter_distribution.rs` extracts `tarball and checksum`, and runs it
      with `bash` in a temporary directory whose `artifacts/` holds
      `ridl-<t>.tar.gz` and `ridlc-<t>.tar.gz` for the five targets, with a fake
      `minisign` first on `PATH` that appends `"$@"` (one line per argument,
      then `--`) and its standard input to a record file and writes
      `<-x value>`. The temporary directory also holds
      `crates/ridl/src/toolchain/mod.rs`: by default a one-line file declaring a
      non-placeholder key; in the placeholder test, a copy of the repository's
      current file. Tests:

  - `signing_step_fails_without_the_secret`: `RIDL_RELEASE_SIGNING_KEY=""`.

    ```rust
    assert!(!status.success());
    assert!(stderr.contains("RIDL_RELEASE_SIGNING_KEY"));
    assert!(!record.exists());
    assert!(minisig_files.is_empty());
    ```

  - `signing_step_signs_each_ridl_archive_with_its_trusted_comment`: key
    `fake-key`, password `fake-password`, `RELEASE_TAG=editor-v0.23.0`.

    ```rust
    assert_eq!(invocations.len(), 5);
    assert!(invocations.contains(&vec!["-S", "-s", key_path, "-m", "artifacts/ridl-x86_64-unknown-linux-gnu.tar.gz", "-t", "ridl 0.23.0 x86_64-unknown-linux-gnu", "-x", "artifacts/ridl-x86_64-unknown-linux-gnu.tar.gz.minisig"]));
    assert!(stdins.iter().all(|s| s == "fake-password\n"));
    assert!(!artifacts_has("ridlc-x86_64-unknown-linux-gnu.tar.gz.minisig"));
    assert!(!any_file_contains(tmp, "fake-key")); // the key file is removed
    ```

  - `signing_step_fails_when_no_archive_is_present`: empty `artifacts/`, secrets
    set: `assert!(!status.success())`.
  - `signing_step_fails_while_the_public_key_is_a_placeholder`: secrets set,
    archives present, `mod.rs` contains `RIDL-RELEASE-KEY-PLACEHOLDER`.

    ```rust
    assert!(!status.success());
    assert!(stderr.contains("RIDL-RELEASE-KEY-PLACEHOLDER"));
    assert!(!record.exists());
    assert!(minisig_files.is_empty());
    ```
  - `release_job_installs_minisign_then_signs_before_upload`: in the `release`
    job, the step order is `download-artifact`, `install minisign`,
    `sign the ridl archives`, `softprops/action-gh-release`, and the `files:`
    list contains `artifacts/*.minisig`.
  - `minisign_verify_notice_accompanies_the_dependency`:
    `THIRD-PARTY-NOTICES.txt` contains
    `"minisign-verify 0.3.0 — Minisign signature verification\nSource: https://github.com/jedisct1/rust-minisign-verify\nLicence: MIT\n"`.

  Mutations killed: drop the `:?` guard; drop the placeholder check, or run it
  after signing; sign `ridl*.tar.gz` (catches `ridlc`); omit `-t`; leave the key
  file.

- [ ] **Step 9: Run them to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test release_signing`. Expected: FAIL
      ("no step named sign the ridl archives", missing notice).

- [ ] **Step 10: Add the workflow steps and the notice.** In the `release` job,
      after `actions/download-artifact`, add step `install minisign`
      (`sudo apt-get update && sudo apt-get install -y --no-install-recommends minisign && minisign -v`;
      tool choice: the Ubuntu `minisign` package, because it reads the key
      password from standard input and needs no build) and step
      `sign the ridl archives` with `env` `RIDL_RELEASE_SIGNING_KEY`,
      `RIDL_RELEASE_SIGNING_KEY_PASSWORD` (both from `secrets`) and
      `RELEASE_TAG: ${{ github.ref_name }}`. Its script: `set -euo pipefail`;
      `: "${RIDL_RELEASE_SIGNING_KEY:?the RIDL_RELEASE_SIGNING_KEY secret is required to sign a release}"`
      and the same for the password; then
      `if grep -qF RIDL-RELEASE-KEY-PLACEHOLDER crates/ridl/src/toolchain/mod.rs; then echo "error: crates/ridl/src/toolchain/mod.rs still holds the RIDL-RELEASE-KEY-PLACEHOLDER public key; a release signed now could not be verified" >&2; exit 1; fi`,
      before any key file is written; `version="${RELEASE_TAG#editor-v}"`; write
      the key to a `mktemp` file removed by an `EXIT` trap; `nullglob` over
      `artifacts/ridl-*.tar.gz` and fail when it matches nothing; for each
      archive derive `target` from the name and run
      `printf '%s\n' "$RIDL_RELEASE_SIGNING_KEY_PASSWORD" | minisign -S -s "$key" -m "$archive" -t "ridl $version $target" -x "$archive.minisig"`.
      Add `artifacts/*.minisig` to the release `files:`. The signing secret
      therefore exists only in the Linux `release` job, not in the five build
      runners; this is the only deviation from the design's step placement. Add
      the notice entry.

- [ ] **Step 11: Write the docs.** Directory record: production requests add the
      `.minisig` URL; bound 4 KiB; prehashed signatures only; the trusted
      comment must equal `ridl <version> <target>`, which stops an older or
      another target's signed archive from being served under a new name; the
      transaction verifies the signature after the checksum and before
      extraction; the checksum paragraph's "not an independently signed
      publisher identity" sentence is replaced. Release-pin chapter: install
      verifies the checksum and the signature. Exit row text.

- [ ] **Step 12: Run all focused checks.** Run
      `cargo test --locked -p ridl-cli --test toolchain`,
      `cargo test --locked -p ridl-cli --test release_signing`,
      `cargo test --locked -p ridl-cli --test cli_reference`,
      `cargo test --locked -p ridl-cli --lib`, `just install-check`,
      `just link-check`, `just doc-path-check`. Expected: PASS.

- [ ] **Step 13: Commit.**
      `feat(ridl): verify minisign signatures of release archives`

### Task 3: Hand a refused terminal command to a compatible installed release

**Files:**

- Create: `crates/ridl/src/handoff.rs`, `crates/ridl/tests/handoff.rs`.
- Modify: `crates/ridl/src/toolchain/selection.rs`,
  `crates/ridl/src/toolchain/mod.rs`, `crates/ridl/Cargo.toml` (`windows-sys`),
  `Cargo.lock`.
- Modify the hook sites (table below): `crates/ridl/src/main.rs`,
  `crates/ridl/src/lock.rs`, `crates/ridl/src/property.rs`,
  `crates/ridl/src/vendor.rs`.
- Modify: `crates/ridl/tests/release_pin.rs` (`run` helper isolation),
  `crates/ridl/tests/toolchain.rs`.
- Docs: `docs/book/release-pin.md` (new section "Running the pinned release from
  a terminal"; replace "Selecting a binary does not run it; invoke the returned
  path explicitly in terminal workflows."), `docs/design/toolchain-directory.md`
  (new section "Terminal hand-off": selection rule, guard, Unix `exec`, Windows
  Job Object, no output on success, the site list without line numbers,
  regression evidence), `docs/book/cli-reference.md` (one sentence under the
  exit table: a command that hands off exits with the selected release's code).

**Interfaces:**

- Consumes Task 1:
  `ridl_cli::toolchain::process_toolchain_root() -> io::Result<PathBuf>`.
- Produces in `ridl_cli::toolchain`:
  - `pub const BUILD_TARGET: &str = env!("RIDL_BUILD_TARGET");`
  - `pub fn select_installed(root: &Path, pins: &[Release], target: &str) -> io::Result<Option<(Release, PathBuf)>>`
    — over `list_for_target`, skipping damaged entries, the highest release with
    `compatible(release, pin)` for every pin; returns the release and its
    canonical binary path; `pins` empty returns `Ok(None)`.
- Produces in `crates/ridl/src/handoff.rs` (binary module, `mod handoff;` in
  `main.rs`):
  - `pub(crate) const GUARD: &str = "RIDL_TOOLCHAIN_HANDOFF";`
  - `pub(crate) enum Install { Never, WhenOptedIn }` (Task 4 gives `WhenOptedIn`
    its behaviour; in this task both behave as `Never`).
  - `pub(crate) fn refusal(diagnostics: &[Diagnostic], sources: &SourceMap) -> Option<ExitCode>`
    = `hand_off(diagnostics, sources, Install::WhenOptedIn)`.
  - `pub(crate) fn hand_off(diagnostics: &[Diagnostic], sources: &SourceMap, install: Install) -> Option<ExitCode>`
    — `None` means "no hand-off; the caller prints its refusal as today".
  - `fn pins(diagnostics: &[Diagnostic], sources: &SourceMap) -> Option<Vec<Release>>`
    — `None` unless every `is_release_refusal` diagnostic is MANI-026 and each
    pin re-parses with
    `parse_release_pin(d.primary.file, sources.text(d.primary.file)?)`; sorted
    and deduplicated.
  - `fn guarded(guard: Option<&OsStr>) -> bool` — `true` when the variable is
    present, whatever its value.
  - `fn child_command(binary: &Path, args: impl IntoIterator<Item = OsString>, running: Release) -> std::process::Command`
    — `Command::new(binary).args(args).env(GUARD, running.to_string())`, no
    other change (working directory and standard streams inherited). Both
    platforms build the child with it, from `std::env::args_os().skip(1)`.
  - `fn exec_failure(release: Release, binary: &Path, error: &io::Error) -> String`
    — the Global Constraints message.
  - `fn run(release: Release, binary: &Path) -> ExitCode` — the only
    platform-specific function. Unix: `child_command(..).exec()`; on return
    print `exec_failure` and return 2. Windows (`#[cfg(windows)]`, kept as small
    as possible): create a Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`,
    spawn `child_command(..)`, assign the child, call
    `SetConsoleCtrlHandler(None, 1)`, wait, then `std::process::exit(code)` with
    the child's full exit code; a spawn error prints `exec_failure` and
    returns 2.
  - Unit tests in `handoff.rs` (`#[cfg(test)]`, every platform, run by
    `cargo test --locked -p ridl-cli --bin ridl handoff`):
    `pins_reparse_each_mani_026_and_refuse_other_codes` (two MANI-026 with pins
    `0.23.4` and `0.23.1` plus a duplicate give `[0.23.1, 0.23.4]`; adding a
    MANI-024 gives `None`; a MANI-026 whose file text is missing gives `None`),
    `guard_is_present_for_any_value` (`Some("")` and `Some("x")` are `true`,
    `None` is `false`), and
    `child_command_passes_arguments_and_sets_only_the_guard` (`get_args()`
    equals the given arguments including `"src dir"`, `get_envs()` is exactly
    `[(GUARD, Some("0.22.5"))]`, `get_current_dir()` is `None`). Mutations
    killed: drop the MANI-024 check; treat an empty guard as absent; join
    arguments into one string; set a working directory.
  - Order in `hand_off`: guard present → `None`; `pins` → `None` on `None` or
    empty; `process_toolchain_root()` and `select_installed(.., BUILD_TARGET)`,
    where an error counts as "nothing installed"; a selection → `Some(run(..))`;
    otherwise `None` (Task 4 inserts the install branch here).

**Hook sites** (each call is
`if let Some(code) = handoff::refusal(&d, &s) { return code; }` placed after the
refusal is detected and before any `errprint!`, `render`, `out::write`, `finish`
or `finish_check`; `finish` and `exit_code` are not hooks):

| Site                                                           | Commands                                                | Evidence that nothing was written, fetched or read from stdin                                                                                                                                                                                     |
| -------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `main.rs:959` `run_check`, before `finish_check` (960)         | `check` (text, JSON, SARIF)                             | `ridlc::run_check_unlevelled` returns through `load_and_check`, which stops at `ridlc/src/lib.rs:2817`; the core loader stops before lockfiles and remotes (`release_refusal_precedes_members_overlays_and_remotes`).                             |
| `main.rs:1056` `run_build`, `Ok(run)` arm, before `finish`     | `build`                                                 | `run_build_inner` returns at `ridlc/src/lib.rs:1317`, before emits and before the catalog closure; `release_refusal_exits_two_and_preserves_outputs` checks `out/`.                                                                               |
| `main.rs:624` `release_preflight_exit`, `Ok(Some(..))` arm     | `describe` (647), `baseline` (1094), `fmt` (3478, 3501) | Reads only root manifests (`read_root_release`). `fmt` at 3501 has only listed directories; `baseline` runs before staging is reserved.                                                                                                           |
| `main.rs:867` `report_diff_side_error`, `Compile` arm          | source `diff` sides                                     | The refusal comes from `source_release_preflight` (`ridlc/src/diff_side.rs:175`) or `compile_workspace_with` (`Frozen::Yes`); nothing is printed before the report. The arm also carries ordinary compile errors; `pins` returns `None` for them. |
| `lock.rs:126` `run_lock`                                       | `lock`                                                  | `compile_workspace` uses `Frozen::Yes` (`ridlc/src/lib.rs:478`); allocation writes come later.                                                                                                                                                    |
| `property.rs:208` `run`                                        | `test`                                                  | `compile_workspace`, as above; sampling comes later.                                                                                                                                                                                              |
| `vendor.rs:61` `run`, a new check before `refused` is computed | `vendor`                                                | `RemoteAccess::refresh` is not reached: the root refusal precedes remotes and the lockfile rewrite in the core loader; `write_vendor` comes later. Only `is_release_refusal` triggers the hook, never MANI-101 alone.                             |
| `vendor.rs:142` `run_check`                                    | `vendor --check`                                        | `load_workspace` with the frozen loader; `read_lockfile` and `check_vendor` come later.                                                                                                                                                           |

No site reads standard input: `grep -rn stdin crates/ridl/src` finds only the
`ridl lsp` documentation. **`fmt` can report more than one MANI-026**: its
second preflight (`main.rs:3501`) checks every root found by the walk, and
`release_fmt_reports_both_refusing_roots_before_writing` already observes two.
The multi-pin test therefore stays, on `fmt`.

- [ ] **Step 1: Isolate existing refusal tests.** In `tests/release_pin.rs`,
      make `run` set `RIDL_TOOLCHAIN_DIR` to an empty `TempDir` held for the
      call and remove `RIDL_TOOLCHAIN_HANDOFF` and
      `RIDL_TOOLCHAIN_AUTO_INSTALL`. Run
      `cargo test --locked -p ridl-cli --test release_pin`. Expected: PASS.

- [ ] **Step 2: Write the failing selection test.** First add the inert
      `select_installed` returning `Ok(None)`. Add to `tests/toolchain.rs`
      `toolchain_select_installed_requires_every_pin`: installed complete
      `0.23.3`, `0.23.6`, `0.24.0`, damaged `0.23.9`.

  ```rust
  assert_eq!(select(&["0.23.1", "0.23.4"]), Some(("0.23.6", bin_0_23_6)));
  assert_eq!(select(&["0.23.7"]), None);
  assert_eq!(select(&["0.23.1"]), Some(("0.23.6", bin_0_23_6)));
  assert_eq!(select(&[]), None);
  ```

  Mutations killed: `any` instead of `all`; accept damaged entries; pick the
  lowest. Run
  `cargo test --locked -p ridl-cli --test toolchain toolchain_select_installed`.
  Expected: FAIL. Implement; rerun. Expected: PASS.

- [ ] **Step 3: Write the failing hand-off tests.** Add the inert `handoff.rs`
      (`refusal` returns `None`) and the eight hook calls (inert, so behaviour
      is unchanged). Create `tests/handoff.rs` with `#![cfg(unix)]`. Helper
      `fake_release(root: &Path, version: &str)` writes
      `versions/<version>/ridl` (mode 0755), `LICENSE`,
      `THIRD-PARTY-NOTICES.txt` and an empty `.complete`. The script:

  ```sh
  #!/bin/sh
  r="$RIDL_TEST_RECORD"
  printf '%s\n' "<version>" > "$r/version"
  for a in "$@"; do printf '%s\0' "$a"; done > "$r/argv"
  pwd -P > "$r/cwd"
  cat > "$r/stdin"
  printf '%s' "${RIDL_TOOLCHAIN_HANDOFF-unset}" > "$r/guard"
  echo "child stdout"; echo "child stderr" >&2
  exit 7
  ```

  A helper `run(ws, root, args, envs)` sets `RIDL_TEST_RUNNING_RELEASE=0.22.5`,
  `RIDL_TOOLCHAIN_DIR=root`, `RIDL_TEST_RECORD=<record dir outside ws>`,
  `HOME=<temp home>`, removes the guard and auto-install variables, writes
  `stdin probe\n` to standard input. The workspace fixture is the one of
  `release_pin.rs::workspace` with pin `"0.23.1"` (`a.typl`, broken `ridl.lock`,
  `out/sentinel`) plus an `[imports]` entry naming an unreachable `file://` URL.

  - `handoff_runs_the_selected_release_for_every_enforcing_command`, with
    `fake_release(root, "0.23.4")`, for each of: `check .`,
    `check . --format json`, `build . --out-dir out`, `fmt .`, `fmt . --check`,
    `lock .`, `baseline . --out out`, `test .`, `vendor .`, `vendor . --check`,
    `diff . a.typl`, `diff a.typl .`, `describe a.typl`:

    ```rust
    assert_eq!(out.status.code(), Some(7), "{args:?}");
    assert_eq!(out.stdout, b"child stdout\n", "{args:?}");
    assert_eq!(out.stderr, b"child stderr\n", "{args:?}");
    assert_eq!(recorded_argv, args, "{args:?}");
    assert_eq!(recorded_cwd, ws.canonicalize()?, "{args:?}");
    assert_eq!(recorded_stdin, b"stdin probe\n", "{args:?}");
    assert_eq!(recorded_guard, "0.22.5", "{args:?}");
    assert_eq!(tree(ws), before, "{args:?}");
    assert!(!home.join(".ridl").exists());
    ```

    Mutation killed, per row: delete that site's hook call (the row exits 2).

  - `handoff_skips_under_the_guard`: `RIDL_TOOLCHAIN_HANDOFF=0.23.4`, `check .`.

    ```rust
    assert_refusal(&out, "MANI-026");
    assert!(record_dir_is_empty);
    ```

    Mutation killed: ignore the guard.

  - `handoff_selects_the_highest_release_compatible_with_every_pin`: `fmt .`
    over two roots `a` (pin `0.23.1`) and `b` (pin `0.23.4`) under a parent
    without a manifest; installed `0.23.3`, `0.23.6`, `0.24.0`, damaged
    `0.23.9`.

    ```rust
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(recorded_version, "0.23.6");
    // second root directory with only 0.23.3 installed
    assert_eq!(out2.status.code(), Some(2));
    assert_eq!(stderr(&out2).matches("error[MANI-026]").count(), 2);
    assert!(record2_is_empty);
    ```

  - `handoff_is_skipped_when_any_refusal_is_not_mani_026` (Review Focus 1):
    `fmt .` over root `a` (pin `42`) and `b` (pin `0.23.1`), `0.23.4` installed.

    ```rust
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("error[MANI-024]"));
    assert!(stderr(&out).contains("error[MANI-026]"));
    assert!(record_is_empty);
    ```

    Mutation killed: filter MANI-026 pins and ignore the other codes.

  - `handoff_preserves_arguments_with_spaces_and_a_root_with_spaces` (Review
    Focus 2): toolchain root `<tmp>/tool root`, workspace `<tmp>/work space`
    with a subdirectory `src dir` holding `a.typl`; args
    `["check", "src dir", "--format", "json"]`.

    ```rust
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(recorded_argv, ["check", "src dir", "--format", "json"]);
    ```

  - `handoff_reports_an_exec_failure`: `0.23.4` installed with mode 0644.

    ```rust
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).starts_with(&format!("error: could not run ridl 0.23.4 at {}: ", bin.display())));
    assert!(record_is_empty);
    ```

    Mutation killed: return `None` after an `exec` error (the refusal would
    print instead).

- [ ] **Step 4: Run them to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test handoff`. Expected: FAIL with exit
      `2` where `7` is expected (inert `refusal`).

- [ ] **Step 5: Implement `handoff.rs` as specified in Interfaces.** Add
      `windows-sys` under `[target.'cfg(windows)'.dependencies]` with the four
      features of the Global Constraints. Re-export `select_installed` and
      `BUILD_TARGET`.

- [ ] **Step 6: Run them to verify they pass.** Run
      `cargo test --locked -p ridl-cli --test handoff`,
      `cargo test --locked -p ridl-cli --test release_pin`,
      `cargo test --locked -p ridl-cli --test toolchain` and
      `cargo test --locked -p ridl-cli --bin ridl handoff`. Expected: PASS. No
      Windows CI job exists and this change adds none (Task 7 drafts the
      follow-up issue). If the `x86_64-pc-windows-msvc` target and a C toolchain
      for `ring` are installed locally, also run
      `cargo check --locked -p ridl-cli --target x86_64-pc-windows-msvc`;
      otherwise report that the Windows branch was not compiled.

- [ ] **Step 7: Write the docs.** Release-pin chapter section: which commands
      hand off (the enforcing list), the selection rule, that arguments,
      directory, environment and the three standard streams pass through, the
      exit code, the guard and its value, that MANI-024 and MANI-025 never hand
      off, and that `ridl toolchain which` still selects without running.
      Directory record section as listed in Files. Run `just link-check` and
      `just doc-path-check`. Expected: PASS.

- [ ] **Step 8: Commit.**
      `feat(ridl): hand a refused workspace command to a compatible installed release`

### Task 4: Hand off `ridl mcp`, opt-in automatic install, and the `which` hint

**Files:**

- Modify: `crates/ridl/src/handoff.rs`, `crates/ridl/src/main.rs` (`run_mcp` at
  748, the `which` missing message at 463, the `install` call in
  `run_toolchain`).
- Test: `crates/ridl/tests/handoff.rs`,
  `crates/ridl/tests/toolchain.rs:928-933`.
- Docs: `docs/book/release-pin.md` (automatic install paragraph; `ridl mcp`
  hand-off in "Editors and agent tools"), `docs/design/toolchain-directory.md`
  (automatic install inside "Terminal hand-off"),
  `docs/technotes/toolchain-distribution.md` (the "Versioned toolchains"
  sentence "with no CLI, manifest or environment override" gains: release builds
  have none; debug builds read `RIDL_TEST_RELEASE_ORIGIN` and
  `RIDL_TEST_RELEASE_PUBLIC_KEY` in integration tests, as they read
  `RIDL_TEST_RUNNING_RELEASE`).

**Interfaces:**

- Consumes Task 2: `install(root, release, target, base_url, public_key)`,
  `RELEASE_BASE_URL`, `RELEASE_PUBLIC_KEY`; support `fixture_for`,
  `test_public_key`.
- Consumes Task 3: `hand_off`, `Install`, `GUARD`, `select_installed`,
  `BUILD_TARGET`, `process_toolchain_root`, test helpers in `tests/handoff.rs`.
- Produces in `handoff.rs`:
  - `pub(crate) const AUTO_INSTALL: &str = "RIDL_TOOLCHAIN_AUTO_INSTALL";`
  - `pub(crate) fn release_origin() -> (String, String)` —
    `(RELEASE_BASE_URL, RELEASE_PUBLIC_KEY)`; under `cfg(debug_assertions)` each
    is replaced by the nonempty value of `RIDL_TEST_RELEASE_ORIGIN` /
    `RIDL_TEST_RELEASE_PUBLIC_KEY`. Both `run_toolchain`'s install and the
    automatic install use it.
  - `pub(crate) fn mcp() -> Option<ExitCode>` — guard present → `None`;
    `release_preflight`-equivalent over `.`:
    `read_root_release(Path::new("."), None)`, any error → `None`;
    `check_release_pin(file, text, running_release())` must be MANI-026; then
    `hand_off(&[d], &sources, Install::Never)`.
- `hand_off` with `Install::WhenOptedIn`, when nothing is selected: if
  `std::env::var_os(AUTO_INSTALL) == Some("1")`, the guard is absent and
  `pins.len() == 1`: print `installing ridl <pin>…` on stderr, call `install`
  with `release_origin()` and `BUILD_TARGET`; on success print
  `installed ridl <pin> at <binary>` and return `Some(run(pin, &binary))`; on
  failure print `error: <install error>` and return `None` (the caller then
  prints the refusal and exits 2). A toolchain root error is an install failure
  here.
- `which` text for `missing`:
  ``No compatible ridl toolchain is installed; run `ridl toolchain install <pin>`, or set RIDL_TOOLCHAIN_AUTO_INSTALL=1 so that a workspace command installs it.``

- [ ] **Step 1: Write the failing tests.** Fixture origin:
      `fixture_for(origin, "0.23.1", BUILD_TARGET, entries)` whose binary entry
      is the Task 3 recording script with version `0.23.1`; pass
      `RIDL_TEST_RELEASE_ORIGIN=file://<origin>` and
      `RIDL_TEST_RELEASE_PUBLIC_KEY=<test_public_key()>` in every test below (so
      a wrong "on" really installs). In `tests/handoff.rs`:

  - `mcp_hands_off_before_reading_standard_input`: pin `0.23.1`, `0.23.4`
    installed, `ridl mcp` with standard input
    `{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n`.

    ```rust
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(recorded_argv, ["mcp"]);
    assert_eq!(recorded_stdin, initialize_line.as_bytes());
    ```

    Mutation killed: place the call after the Tokio runtime and serve start (the
    child would see empty input), or remove it.

  - `mcp_never_installs_and_respects_the_guard`: (a) nothing installed,
    `RIDL_TOOLCHAIN_AUTO_INSTALL=1`, empty standard input; (b) `0.23.4`
    installed and the guard set.

    ```rust
    assert!(!root.join("versions/0.23.1").exists());
    assert!(record_is_empty_a);
    assert!(record_is_empty_b);
    ```

    Mutation killed: pass `Install::WhenOptedIn` from `mcp`; ignore the guard in
    `mcp`.

  - `auto_install_is_off_unless_the_variable_is_exactly_one`: `check .`, no
    installed release, for values unset, `""`, `"0"`, `"true"`, `" 1"`, `"1 "`,
    `"11"`.

    ```rust
    assert_refusal(&out, "MANI-026");
    assert!(!root.join("versions/0.23.1").exists(), "{value:?}");
    assert!(!stderr(&out).contains("installing ridl"), "{value:?}");
    ```

  - `auto_install_with_one_installs_and_hands_off`: value `1`.

    ```rust
    assert_eq!(out.status.code(), Some(7));
    assert_eq!(stderr(&out), format!("installing ridl 0.23.1…\ninstalled ridl 0.23.1 at {}\nchild stderr\n", bin.display()));
    assert!(root.join("versions/0.23.1/.complete").exists());
    assert_eq!(recorded_version, "0.23.1");
    ```

  - `auto_install_requires_exactly_one_distinct_pin`: `fmt .` over roots pinned
    `0.23.1` and `0.23.4`, value `1`, fixtures for both: exit 2, two refusals,
    no `versions/` entries.
  - `auto_install_never_runs_under_the_guard`: value `1` and guard set: exit 2,
    no `versions/0.23.1`.
  - `auto_install_failure_prints_the_error_before_the_refusal`: value `1`,
    origin without assets.

    ```rust
    let lines: Vec<_> = stderr(&out).lines().collect();
    assert_eq!(lines[0], "installing ridl 0.23.1…");
    assert!(lines[1].starts_with("error: "));
    assert!(lines[2..].iter().any(|l| l.starts_with("error[MANI-026]")));
    assert_eq!(out.status.code(), Some(2));
    ```

  Mutations killed: compare with `is_some()` instead of `== "1"`; drop the guard
  check; drop the one-pin check; return `Some(2)` instead of `None` after a
  failed install (the refusal would be missing).

  In `tests/toolchain.rs`, change the expected `which` text at lines 930-933 to
  the new sentence (mutation killed: old text).

- [ ] **Step 2: Run them to verify they fail.** Run
      `cargo test --locked -p ridl-cli --test handoff mcp_ auto_install_` and
      `cargo test --locked -p ridl-cli --test toolchain toolchain_cli_streams_and_exit_codes`.
      Expected: FAIL on exit codes, missing install directories and the message.

- [ ] **Step 3: Implement** `release_origin`, the install branch of `hand_off`,
      `mcp` and the call `if let Some(code) = handoff::mcp() { return code; }`
      as the first statement of `run_mcp`, and the new `which` text.

- [ ] **Step 4: Run them to verify they pass.** Run
      `cargo test --locked -p ridl-cli --test handoff`,
      `cargo test --locked -p ridl-cli --test toolchain`,
      `cargo test --locked -p ridl-cli --test servers`. Expected: PASS.

- [ ] **Step 5: Write the docs** listed in Files. Run `just link-check` and
      `just doc-path-check`. Expected: PASS.

- [ ] **Step 6: Commit.**
      `feat(ridl): hand off ridl mcp and install a pinned release on request`

### Task 5: Installer scripts — relative root, old script stages, `mv` failure message

**Files:**

- Modify: `install.sh` (`toolchain_root` at 83-98; `main` at 180-210),
  `install.ps1` (`Get-ToolchainRoot` at 69-73; `Main` around 178-186).
- Modify: `justfile` `install-check` recipe (2864-3260).
- Docs: `docs/book/getting-started.md` (one sentence after the variable table:
  `RIDL_TOOLCHAIN_DIR` must be absolute), `docs/book/release-pin.md` (the
  relative-override sentence covers the scripts),
  `docs/design/toolchain-directory.md` ("Install scripts and the PATH copy":
  stage cleanup rule; regression evidence), `AGENTS.md` (`install-check`
  description gains "relative root refusal, removal of old script stages, a
  failed `mv`").

**Interfaces:**

- Consumes: nothing from other tasks.
- Produces: script behaviour only.
  - Relative root: `install.sh` prints
    `error: RIDL_TOOLCHAIN_DIR must be an absolute path, not '<value>'` and
    exits 1 (the script's failure status); `install.ps1` throws the same text.
    In PowerShell a value is absolute when `[IO.Path]::IsPathRooted($v)` and,
    where `[IO.Path]::DirectorySeparatorChar -eq '\'`, it also matches
    `^[A-Za-z]:[\\/]` or `^[\\/]{2}` (Windows PowerShell 5.1 has no
    `IsPathFullyQualified`). `LOCALAPPDATA` keeps `GetFullPath`.
  - Old stages: before creating its own stage, a script removes each directory
    directly under `versions/` named `.tmp-script-<normalized>-<digits>`
    (nonempty all-digit suffix) whose modification time is more than 24 hours
    old. Bash:
    `find "$root/versions" -mindepth 1 -maxdepth 1 -type d -name ".tmp-script-$normalized-*" -mmin +1440`,
    then a `case` check of the suffix. PowerShell:
    `Get-ChildItem -Directory -Force -Filter`, `-match '^\d+$'` on the suffix,
    `LastWriteTimeUtc -lt (Get-Date).ToUniversalTime().AddHours(-24)`. The
    script's own exact name is still removed as today.
  - `mv` failure: capture `mv -n`'s status; then (1) complete destination →
    reuse; (2) destination exists → today's "appeared" message; (3) otherwise
    `error: could not move the staged release to $root/versions/ (mv exit <status>)`.

- [ ] **Step 1: Write the failing `install-check` cases.**

  - Replace the relative-root lines at the end of
    `install_platform_roots_and_spaces` with the new function
    `install_refuses_a_relative_toolchain_dir`:

    ```sh
    if (cd "$scratch" && RIDL_TOOLCHAIN_DIR="relative root" bash "$repo/install.sh") 2>"$scratch/relative.err"; then return 1; fi
    grep -qF "RIDL_TOOLCHAIN_DIR must be an absolute path" "$scratch/relative.err"
    test ! -e "$scratch/relative root"
    ```

  - `install_removes_old_script_stages_of_its_version`: under a fresh root
    create, then age with
    `python3 -c 'import os,sys,time; t=time.time()-25*3600; os.utime(sys.argv[1],(t,t))'`:
    old `.tmp-script-0.23.0-111` (removed), fresh `.tmp-script-0.23.0-222`
    (kept), old `.tmp-script-0.22.9-333` (kept), old `.tmp-script-0.23.0-4x4`
    (kept), old `.tmp-0.23.0-555` (kept), old regular file
    `.tmp-script-0.23.0-666` (kept). Run `bash install.sh`, then
    `test ! -e .../.tmp-script-0.23.0-111` and `test -e` for each kept name, and
    `test -f "$root/versions/0.23.0/.complete"`.
  - `install_reports_a_failed_mv`: a fake `mv` first on `PATH` that, for `-n`,
    exits 1 without creating anything, otherwise `exec /bin/mv`.

    ```sh
    if PATH="$scratch/fail-mv:$PATH" RIDL_TOOLCHAIN_DIR="$root" RIDL_INSTALL_DIR="$dest" bash install.sh 2>"$scratch/mv.err"; then return 1; fi
    grep -qF "could not move the staged release" "$scratch/mv.err"
    ! grep -qF "appeared" "$scratch/mv.err"
    test ! -e "$root/versions/0.23.0"; test ! -e "$dest/ridl"
    test -z "$(find "$root/versions" -name '.tmp-*')"
    ```

  - In the first PowerShell block, `install_refuses_a_relative_toolchain_dir`:
    `$env:RIDL_TOOLCHAIN_DIR = "relative root"`, `./install.ps1` must throw a
    message containing `RIDL_TOOLCHAIN_DIR must be an absolute path`, and
    `relative root` must not exist. In the second block,
    `install_removes_old_script_stages_of_its_version` with
    `(Get-Item $old).LastWriteTimeUtc = (Get-Date).ToUniversalTime().AddHours(-25)`
    and the same kept set as Bash.

  Add the three Bash names to the `for case in` list. Mutations killed: keep
  `root="$PWD/$root"`; drop `-mmin +1440` (the fresh stage is removed); drop the
  digit check; match any version; keep the old "appeared" branch.

- [ ] **Step 2: Run to verify it fails.** Run `just install-check`. Expected:
      FAIL at `install_refuses_a_relative_toolchain_dir` (the install succeeds),
      and after that case is fixed, at the next new case. Without `pwsh` the
      PowerShell cases do not run; say so in the report.

- [ ] **Step 3: Implement the three script changes** in `install.sh` and
      `install.ps1` as specified in Interfaces.

- [ ] **Step 4: Run to verify it passes.** Run `just install-check`. Expected:
      PASS, every case name printed.

- [ ] **Step 5: Write the docs** listed in Files. Run `just link-check` and
      `just doc-path-check`. Expected: PASS.

- [ ] **Step 6: Commit.**
      `fix(repo): refuse a relative toolchain directory and remove old script stages in the installers`

### Task 6: VS Code Restricted Mode prompt and absolute PATH lookup

**Files:**

- Create: `editors/vscode/src/restrictedMode.ts`,
  `editors/vscode/src/restrictedMode.test.ts`.
- Modify: `editors/vscode/src/binaryResolution.ts`,
  `editors/vscode/src/binaryResolution.test.ts`,
  `editors/vscode/src/toolchainSelection.ts`, `editors/vscode/src/extension.ts`,
  `editors/vscode/src/extension.test.ts`,
  `editors/vscode/src/contributions.test.ts:104-113`,
  `editors/vscode/package.json` (`capabilities`; the `test` script gains
  `out/restrictedMode.test.js`).
- Docs: `editors/vscode/README.md` (Restricted Mode behaviour, PATH rule),
  `docs/book/release-pin.md` ("Editors and agent tools": replace the paragraph
  that starts "The extension declares no support for untrusted workspaces"),
  `docs/design/toolchain-directory.md` (regression evidence: rename
  `auto_install_is_application_scoped_and_trust_capability_is_absent`).

**Interfaces:**

- Consumes: the CLI contracts `ridl toolchain which --json <dir>` (schema 1) and
  `ridl toolchain install <pin>`; no Rust change.
- `package.json`:
  `"capabilities": { "untrustedWorkspaces": { "supported": "limited", "description": "In Restricted Mode, RIDL starts no language server and no MCP server. It only checks whether the release a workspace pins is installed, and offers to install it.", "restrictedConfigurations": ["ridl.serverPath", "ridl.toolchain.autoInstall"] } }`.
- `binaryResolution.ts`:
  - `export type BinarySource = "setting" | "bundled" | "path" | "none";`
  - `ResolveInput` gains `readonly pathValue: string | undefined;`
  - `export function findOnPath(pathValue: string | undefined, platform: NodeJS.Platform, exists: (file: string) => boolean): string | undefined`
    — split on `;` for `win32` and `:` otherwise; skip every entry that is not
    absolute for that platform (`path.win32.isAbsolute` /
    `path.posix.isAbsolute`, so empty and `.` entries are skipped); return the
    first `join(entry, "ridl.exe" | "ridl")` that exists.
  - `resolveBinary`: third tier returns `{ command: found, source: "path" }`, or
    `{ command: bundledBinaryPath(..), source: "none" }` when nothing is found,
    so no spawn ever searches the working directory.
- `toolchainSelection.ts`: `ToolchainSelector` gains public
  `which(command: string, directory: string): Promise<WhichReport>` (the current
  private `probe`, unchanged in behaviour) and
  `install(command: string, pin: string, directory: string): Promise<void>` (the
  current in-flight-shared install under `progress`). `select` uses both; a
  `"none"` resolver reports
  `No ridl binary: the extension has no bundled binary and no absolute PATH entry holds ridl.`
  once and returns the fallback.
- `restrictedMode.ts`:
  - `export const INSTALL = "Install"; export const MANAGE_TRUST = "Manage Workspace Trust";`
  - `export function missingMessage(pin: string): string` →
    `This workspace requires ridl ${pin}, which is not installed.`
  - `export interface RestrictedEffects { prompt(message: string, buttons: readonly string[]): Promise<string | undefined>; manageTrust(): Promise<void>; log(message: string): void; }`
  - `export class RestrictedPrompts { constructor(selector: Pick<ToolchainSelector, "which" | "install">, bundled: string | undefined, effects: RestrictedEffects); check(directory: string): Promise<void>; settled(): Promise<void>; }`
    — `bundled` undefined: log once, never run anything. `check` runs `which`
    with the bundled binary only; on `missing` it prompts once per pin (a set of
    prompted pins) without awaiting the answer; `Install` calls
    `selector.install(bundled, pin, directory)` at most once per pin;
    `Manage Workspace Trust` calls `manageTrust`. Errors are logged. `settled`
    resolves when every answer handler has finished (for tests).
- `extension.ts`: `activate` checks `vscode.workspace.isTrusted`. When false, it
  builds `RestrictedPrompts` with `bundledBinaryPath` when it exists, runs
  `check` for the directory of each `vscode.workspace.findFiles("**/ridl.toml")`
  result and of each opened RIDL document (current and future), registers no MCP
  provider and creates no client, reads neither `ridl.serverPath` nor
  `ridl.toolchain.autoInstall`, and subscribes
  `vscode.workspace.onDidGrantWorkspaceTrust` to run the normal activation once.
  `manageTrust` is `vscode.commands.executeCommand("workbench.trust.manage")`.
  Trusted activation passes `pathValue: process.env.PATH ?? process.env.Path`.

- [ ] **Step 1: Write the failing unit tests.**

  - `binaryResolution.test.ts`, `path_lookup_uses_only_absolute_entries`:

    ```ts
    assert.equal(findOnPath(".:bin::/abs", "linux", (f) => f !== "/abs/ridl" ? f === "bin/ridl" || f === "ridl" : true), "/abs/ridl");
    assert.equal(findOnPath(".;C:\\tools;relative", "win32", (f) => f === "C:\\tools\\ridl.exe" || f === "ridl.exe"), "C:\\tools\\ridl.exe");
    assert.equal(findOnPath(".;relative", "win32", () => true), undefined);
    assert.deepEqual(resolveBinary({ ...base, pathValue: ".", exists: () => false }), { command: bundledBinaryPath(base.extensionPath, base.platform), source: "none" });
    ```

    Update `tier 3: PATH lookup ...` to the absolute result. Mutation killed:
    accept relative entries; return `"ridl"` when nothing is found.

  - `restrictedMode.test.ts`, with a fake selector recording calls and fake
    effects whose `prompt` returns a promise the test resolves:
    - `restricted_missing_release_prompts_without_installing`: `which` reports
      `missing` pin `0.23.1` →
      `prompts == [[missingMessage("0.23.1"), ["Install", "Manage Workspace Trust"]]]`,
      `installs == []` before any answer.
    - `restricted_prompt_is_shown_once_per_release_across_roots` (Review Focus
      5): two directories with distinct manifests, both `missing` `0.23.1`;
      answer the prompt `Install`, then call `check` again for both and answer
      any new prompt `Install` → `prompts.length == 1`, `installs.length == 1`.
    - `restricted_manage_trust_opens_the_trust_editor`: answer
      `Manage Workspace Trust` → `manageTrust` called once, no install.
    - `restricted_without_a_bundle_runs_nothing`: `bundled` undefined →
      `whichCalls == 0`.
    - `restricted_compatible_and_invalid_do_not_prompt`.

    Mutations killed: prompt per directory instead of per pin; install on prompt
    display; run `which` with a non-bundled command.

  - `contributions.test.ts`: rename the test to
    `auto_install_is_application_scoped_and_trust_is_limited` and assert the
    exact `capabilities` object above.

- [ ] **Step 2: Run them to verify they fail.** First add inert exports
      (`findOnPath` returns `undefined`; `RestrictedPrompts.check` does
      nothing). Run `npm test` in `editors/vscode`. Expected: FAIL on the
      assertions above.

- [ ] **Step 3: Implement** `findOnPath`, the `resolveBinary` change, the two
      public `ToolchainSelector` methods, `RestrictedPrompts`, and the
      `capabilities` block.

- [ ] **Step 4: Write the failing activation tests** in `extension.test.ts`. The
      host fake gains `workspace.isTrusted` (default `true`),
      `workspace.onDidGrantWorkspaceTrust: register("trust")`,
      `workspace.findFiles` (returns `[uri("/work/ridl.toml")]`) and
      `commands.executeCommand` (records its arguments). `resetHost` sets
      `process.env.PATH` to a temporary directory holding an empty `ridl` file;
      the existing assertions that expect `"ridl"` expect that absolute path
      instead. Restricted tests pass an `extensionPath` whose `bin/ridl` exists.
  - `restricted_mode_starts_no_client_and_no_mcp_definition`:
    `isTrusted = false`, `autoInstall = true`, `missingRelease = true`.

    ```ts
    assert.equal(clients.length, 0);
    assert.equal(provider, undefined);
    assert.ok(whichBinaries.every((b) => b === bundled));
    assert.equal(installCalls, before);
    assert.deepEqual(restrictedPrompts, [["Install", "Manage Workspace Trust"]]);
    assert.ok(!configurationReads.some(([name]) => name === "serverPath" || name === "toolchain.autoInstall"));
    ```

  - `granting_trust_runs_normal_activation`: after `fire("trust")`, one client
    exists and `provider` is defined.

  Mutations killed: skip the `isTrusted` branch; register the MCP provider
  before the branch; read `serverPath` in Restricted Mode.

- [ ] **Step 5: Run to verify they fail, implement the `activate` branch, run to
      verify they pass.** Run `npm test` in `editors/vscode` each time.
      Expected: FAIL, then PASS.

- [ ] **Step 6: Write the docs** listed in Files. Run `just link-check` and
      `just doc-path-check`. Expected: PASS.

- [ ] **Step 7: Commit.**
      `feat(editors): offer to install a missing release in Restricted Mode`

### Task 7: Amend the records

**Files:**

- Modify: `docs/decisions/ADR-0036-workspace-toolchain-pins.md`,
  `docs/decisions/README.md` (the ADR-0036 entry at 748-754),
  `docs/specification/ridl-family-overview.md` (ADR table row at 42 and ledger
  row 45), `docs/decisions/ADR-0010-cli-conventions.md` (decision 1: the exit
  status of a command that hands off),
  `docs/technotes/toolchain-distribution.md` (new section "Signed release
  archives"; "Cutting a release" note).

**Interfaces:**

- Consumes: the behaviour of Tasks 1-6 as built. Every statement added must be
  checked against the code it describes; cite the file, not the task.

- [ ] **Step 1: Amend ADR-0036 in place.** `## Status`: add a dated line
      "Amended 2026-11-25 by
      [driftsys/ridl#1477](https://github.com/driftsys/ridl/issues/1477):
      decisions 6, 8, 9 and 10, the Restricted Mode alternative and the
      consequences." Decision 6: uninstall, minisign signatures with an embedded
      key and the exact trusted comment, the relative-root refusal. Decision 8:
      limited untrusted support, restricted settings, prompt only, no server and
      no MCP definition in Restricted Mode, absolute PATH entries only. Decision
      9: terminal-side `ridl mcp` hands off from the current directory before it
      serves and never installs; the window still has one MCP definition.
      Decision 10: hand-off, automatic install, uninstall, signing and the
      prompt are implemented; plugin pins and automatic pin updates stay out of
      scope; #1478 is unchanged. New decision 11: terminal hand-off at the
      refusal site (rule, guard, Unix `exec`, Windows Job Object) and the opt-in
      automatic install. Alternatives: "Activate in Restricted Mode just to
      prompt" becomes the chosen limited form, with the rejected "run the
      bundled server in Restricted Mode"; add hand-off before dispatch, GitHub
      artifact attestations, and signing the `.sha256` record, each with its
      reason from the design. Consequences: replace "The terminal user invokes
      the selected path explicitly until hand-off is implemented"; add that a
      lost signing key means a new key for the next release while older binaries
      keep verifying older releases.

- [ ] **Step 2: Update the summaries.** README entry and overview ADR-table
      description: add terminal and MCP hand-off, signed archives, uninstall and
      the Restricted Mode prompt; remove "links the deferred work". Ledger row
      45: replace "after Workspace Trust permits activation; terminal and MCP
      hand-off remain deferred (driftsys/ridl#1477)" with the as-built
      statements. ADR-0010 decision 1: a command that hands off exits with the
      selected release's code.

- [ ] **Step 3: Write the technote section.** Which archives are signed and
      where (the `release` job, Ubuntu `minisign` package), the two secrets, the
      trusted comment, that the step fails without the secret, that the
      production key is embedded in `crates/ridl/src/toolchain/mod.rs`, and key
      rotation: a new key applies from the next release; a binary verifies only
      releases signed with the key it embeds; no release is signed with two
      keys. "Cutting a release": the signing secrets must exist before an
      `editor-v` tag.

- [ ] **Step 4: Draft the Windows CI follow-up issue.** Write its text in the
      pull request description (not in a tracked file), under the heading
      "Follow-up issue to file", with the placeholder
      `driftsys/ridl#TBD-windows-ci` that the driving session replaces after
      filing. Title: "ci: run the toolchain and hand-off tests on Windows".
      Body: no CI job runs `cargo test` on Windows, so the `cfg(windows)`
      hand-off in `crates/ridl/src/handoff.rs` (Job Object, console handler,
      exit-code forwarding) and the Windows paths of
      `crates/ridl/tests/toolchain.rs` are compiled only by the release
      workflow's Windows build and are never tested; the job adds a
      `windows-latest` runner that runs
      `cargo test --locked -p ridl-cli --test toolchain --test handoff` and
      `cargo test --locked -p ridl-cli --bin ridl handoff`, with the hand-off
      tests' fake release written as a small compiled test binary instead of a
      shell script; it is a new member of `just build` and of the `gate-parity`
      expected list, or a documented exception. Wherever a shipped record in
      this task states that Windows hand-off is untested (ADR-0036 consequences,
      the directory record), cite `driftsys/ridl#TBD-windows-ci`; the driving
      session replaces every occurrence before merge (`grep -rn TBD-windows-ci`
      must be empty at merge).

- [ ] **Step 5: Verify.** Run `just link-check`, `just doc-path-check`,
      `just story-id-check`, `just check`. Expected: PASS. Read each changed
      sentence against the code it names.

- [ ] **Step 6: Commit.**
      `docs(adr): amend ADR-0036 for hand-off, signing, uninstall and Restricted Mode`

## Self-review record

- Spec coverage: goals 1-6 map to Tasks 3, 4, 4, 1, 2, 6; smaller items 1-6 map
  to Tasks 1 and 5, 6, 5, 5, 1, 4; the design's test list is covered except the
  Windows compiled test binary, which moves to the follow-up issue drafted in
  Task 7 (`driftsys/ridl#TBD-windows-ci`); records to Task 7.
- Deviations from the design, each stated in its task: signing runs in the
  `release` job, not the build step; `install-check`'s script fixture serves no
  `.minisig` because the scripts do not verify signatures (the Rust HTTP test
  inside `install-check` does serve one); debug-only origin and key seams for
  binary tests of the automatic install; the `minisign` crate as a dev-only
  dependency with its new lock entries; the production key is enforced by the
  release workflow's signing step, not by a test.
