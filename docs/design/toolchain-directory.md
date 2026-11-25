# Toolchain directory and selection protocol

This is the as-built protocol for `ridl toolchain` and the versioned install
scripts. [ADR-0036](../decisions/ADR-0036-workspace-toolchain-pins.md) owns the
decisions. The [release pin chapter](../book/release-pin.md) owns compatibility
and selection rules, manifest usage, platform root precedence, command coverage
and editor behavior. This record owns the directory layout, selection schema and
install transaction; it introduces no separate compatibility rule.

## Ownership and layout

`crates/ridl-core/src/release.rs` parses and compares releases. Root discovery
and the early refusal belong to `crates/ridl-core/src/workspace.rs`.
`crates/ridl/src/toolchain/paths.rs` resolves the toolchain root and validates
archive targets; `selection.rs` inventories and selects releases; `install.rs`
downloads and publishes them; `uninstall.rs` removes them.
`crates/ridl/src/toolchain/mod.rs` defines the production origin constant;
`main.rs` passes it with the current executable, running release and build
target, and maps results to command output. Core, IR and descriptor crates do
not install toolchains.

Given the
[configured root](../book/release-pin.md#installing-and-selecting-toolchains),
the layout is:

```text
<root>/versions/
  <MAJOR.MINOR.PATCH>/
    ridl                 # ridl.exe for a Windows target
    LICENSE
    THIRD-PARTY-NOTICES.txt
    .complete
  .<MAJOR.MINOR.PATCH>.lock
  .tmp-<MAJOR.MINOR.PATCH>-<pid>/
  .tmp-script-<MAJOR.MINOR.PATCH>-<pid>/
```

A published release directory uses the canonical numeric version spelling. A
usable entry is a real directory containing a regular target binary and an empty
regular `.complete` file, checked without following symlinks. The license and
notices are required at extraction, but are not revalidated by inventory.
Missing, non-file or nonempty markers and missing/non-file binaries make an
existing entry damaged. Install reports its path and preserves it, including an
existing non-directory at the destination. There is no automatic repair.

Inventory accepts only real directories with canonical version names and sorts
them numerically. It retains damaged entries for `list`, ignores temporary
names, locks, symlinks, noncanonical names and version-named files. An absent
versions directory is empty inventory; other read errors propagate.

## Terminal hand-off

The
[release pin chapter](../book/release-pin.md#running-the-pinned-release-from-a-terminal)
owns the user-visible rules. This section records how the binary applies them.
The code is `crates/ridl/src/handoff.rs`, with `select_installed` in
`selection.rs`.

**Selection.** `select_installed(root, pins, target)` lists installed releases
for the target, skips damaged entries, and returns the highest release that
satisfies `compatible(release, pin)` for every pin, with the canonical path of
its binary. No pins select nothing. The running binary is not a candidate
because it already failed the pin.

**Pins.** The hand-off runs where a command detects the refusal, before the
command renders a diagnostic, writes a file or finishes. It reads no field
beyond the diagnostics: every release refusal in the run has to be MANI-026, and
each pin is parsed again from the text of the file that the diagnostic points
to, with `parse_release_pin`. A diagnostic whose file text is missing, or any
MANI-024 or MANI-025, ends the attempt. The pins are sorted and deduplicated.

**Automatic install.** When no installed release is selected, `hand_off` can
install the pin. It requires the `Install::WhenOptedIn` mode that every
workspace command passes, `RIDL_TOOLCHAIN_AUTO_INSTALL` set to exactly `1`, the
guard absent, and exactly one distinct pin. It prints `installing ridl <pin>…`,
calls `install` with the release origin and key from `release_origin()` and the
build target, prints `installed ridl <pin> at <path>`, and runs the binary that
`install` returns. A failure, including a toolchain root error, prints
`error: <cause>` and returns to the caller, which prints the refusal and exits
2. A selection error counts as nothing selected, so under the opt-in with one
pin it leads to the installation attempt. `ridl mcp` calls `handoff::mcp()` as
the first statement of `run_mcp`, before the Tokio runtime exists and before
standard input is read. It reads the root manifest of the current directory,
requires MANI-026, and passes `Install::Never`.

**Guard.** `RIDL_TOOLCHAIN_HANDOFF` is set for the child to the release that
handed off. A process with the variable present, whatever its value, does not
hand off.

**Starting the child.** One function builds the command for both platforms: the
selected binary, the arguments of the current process after the program name,
and the guard variable. It sets no working directory and no stream, so the child
inherits them. On Unix the process calls `exec`, so nothing remains of it and no
output is produced. If `exec` returns, the process prints
`error: could not run ridl <version> at <path>: <error>` and exits 2. On Windows
the process first creates a Job Object that kills its processes when the job
closes and joins it, so the child it starts afterwards inherits the job. It then
registers a console handler that consumes Ctrl+C and Ctrl+Break in the parent (a
registered handler is not inherited, so the child receives the event normally),
starts the child, waits, and exits with the child's exit code. A failure to
create, configure or join the job, or to start the child, prints the same
message with the failed step named and exits 2, and no child is started without
the job. The Windows branch is not compiled by any gate; no Windows job exists
yet.

**Sites.** The hand-off is called from these places in the `ridl` binary, each
after the refusal is known and before anything is written:

- `ridl check`, before the report is rendered (text, JSON and SARIF).
- `ridl build`, on the run that `ridlc` returns, before the result is printed.
- The shared preflight of the root manifests, used by `describe`, `baseline` and
  both root checks of `fmt`.
- The report of a source side of `diff`, when the side failed to compile.
- `ridl lock`, `ridl test` and `ridl vendor`, on the diagnostics of the
  workspace compile, which loads remote units without the frozen mode.
- `ridl vendor --check`, on the diagnostics of the frozen workspace load.
- `ridl mcp`, in its start-up decision (see "Automatic install" above): it reads
  the root manifest of the current directory, not diagnostics of a command run,
  and never installs.

`ridlc` keeps its refusals, and no site reads standard input.

**Evidence.** `crates/ridl/tests/handoff.rs` runs each enforcing command against
a workspace that has a broken `ridl.lock`, a broken `instances.lock`, an
unreachable import and an output directory. It checks that the selected release
receives the arguments, the working directory, the standard input and the guard,
that the workspace tree and the toolchain root are unchanged, and that the home
directory stays empty. Further tests cover the guard, the selection across two
pins, a refusal that is not MANI-026, arguments and paths with spaces, and an
`exec` failure.

## `which --json`, schema 1

`ridl toolchain which [PATH] --json` defaults to the current directory. It reads
the discovered root pin without enforcing it against the resolver binary. The
[release pin chapter](../book/release-pin.md#installing-and-selecting-toolchains)
owns candidate selection, tie preference and unpinned behavior. No selection
executes a candidate.

Every successful JSON answer contains all eight keys; absent values are JSON
`null`, never omitted. Non-null paths are canonical absolute paths (including
resolved filesystem aliases). Releases in reports use canonical numeric
spelling, including `running`, whose value comes from the numeric running
release rather than an arbitrary display stamp.

| Field        | Type           | Meaning                                                                                   |
| ------------ | -------------- | ----------------------------------------------------------------------------------------- |
| `schema`     | integer        | Exactly `1`.                                                                              |
| `manifest`   | string or null | Discovered root `ridl.toml` path; null only when no manifest exists.                      |
| `pin`        | string or null | Parsed numeric pin, including a below-floor pin; null for no pin or an invalid type/form. |
| `running`    | string         | Numeric release of the resolver process.                                                  |
| `status`     | string         | `compatible`, `unpinned`, `missing` or `invalid`.                                         |
| `path`       | string or null | Selected binary path.                                                                     |
| `source`     | string or null | `running` or `installed` when a binary is selected.                                       |
| `diagnostic` | object or null | When invalid: string fields `code` and `message`.                                         |

The complete state and nullability mapping is:

| Status                  | Manifest          | Pin        | Path            | Source                   | Diagnostic      | Exit |
| ----------------------- | ----------------- | ---------- | --------------- | ------------------------ | --------------- | ---- |
| `compatible`            | root path         | valid pin  | selected binary | `running` or `installed` | null            | 0    |
| `unpinned`              | root path or null | null       | running binary  | `running`                | null            | 0    |
| `missing`               | root path         | valid pin  | null            | null                     | null            | 1    |
| `invalid` (type/form)   | root path         | null       | null            | null                     | MANI-024 object | 1    |
| `invalid` (below floor) | root path         | parsed pin | null            | null                     | MANI-025 object | 1    |

The unpinned state deliberately has a non-null running path and source even when
`manifest` is null. Invalid selection never means MANI-026: a running release
mismatch is a candidate-selection problem, not invalid pin syntax. For example,
an invalid type yields this shape (paths are illustrative):

```json
{
  "schema": 1,
  "manifest": "/work/ridl.toml",
  "pin": null,
  "running": "0.23.2",
  "status": "invalid",
  "path": null,
  "source": null,
  "diagnostic": {
    "code": "MANI-024",
    "message": "`ridl-version` must be a string of the form MAJOR.MINOR.PATCH"
  }
}
```

A missing entry, unreadable manifest, invalid TOML, non-table `package` or
`workspace` values, inventory read failure or required path canonicalization
failure is an input/I/O error: exit **2**, an error on stderr and no JSON answer
on stdout. Root-directory resolution can also fail. Output failures use exit 2.
Without `--json`, success prints the binary path on stdout; a negative answer
prints a sentence on stderr. The management commands do not reject the resolver
merely because its own release is incompatible with the root pin.

## Targets and download source

The CLI build script exports Cargo's `TARGET` as `RIDL_BUILD_TARGET`. Runtime
host probes do not substitute another triple. Archives are published for:

- `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`;
- `x86_64-apple-darwin` and `aarch64-apple-darwin`;
- `x86_64-pc-windows-msvc`.

Install rejects another target with its name in the error before publication.
Selection can still use a compatible running binary on an unsupported target; if
an installed candidate would be needed there, it returns an error instead. The
terminal hand-off treats that error as no selection. Without the automatic
install opt-in, or with more than one distinct pin, it then prints the refusal
unchanged. With the opt-in and one pin, it attempts the installation, and the
install error of an unsupported target is printed before the refusal. Binary
naming follows the supplied target, including `ridl.exe` for Windows.

Production requests use
`https://github.com/driftsys/ridl/releases/download/editor-v<version>/ridl-<target>.tar.gz`
and the same URL with `.sha256` and with `.minisig` appended. The library's
`base_url` parameter allows fixture `file://` and local HTTP sources; a release
build of the CLI always passes its constant origin. A debug build replaces the
origin with the nonempty value of `RIDL_TEST_RELEASE_ORIGIN`, which integration
tests use; no release build, manifest key or command-line option exposes an
override. Each HTTP request has a 30-second connect timeout and a 10-minute
overall timeout. Downloads are bounded to 256 MiB for the archive, 4 KiB for the
checksum record and 4 KiB for the signature file.

The checksum record is one UTF-8 line: 64 hexadecimal SHA-256 digits, then two
spaces or a space followed by `*`, then the exact archive name, with an optional
LF or CRLF. The computed digest must match, case-insensitively. The checksum
verifies consistency with the origin's record only.

The `.minisig` file is a minisign signature of the archive. Only prehashed
signatures verify; a legacy signature is refused. The library takes the public
key as a parameter (`public_key`), and a release build of the CLI passes
`RELEASE_PUBLIC_KEY`. A debug build replaces it with the nonempty value of
`RIDL_TEST_RELEASE_PUBLIC_KEY`. An undecodable key is an error before any
download of a release that is not yet installed, and the error does not name the
origin. The signature's trusted comment must equal `ridl <version> <target>`
exactly, with the version in `MAJOR.MINOR.PATCH` form. This stops a signed
archive of an older release, or of another target, from being served under the
requested name. Archives of `ridlc` are not signed.

## CLI installation transaction

An explicit install request parses the requested release and checks the pin
floor. An omitted version reads the current root pin; absent or invalid pins are
input errors. Neither form compiles sources or enforces compatibility with the
resolver. Success prints the canonical absolute installed binary path. Failures
return exit 2.

1. Validate release and target, create `versions/` and canonicalize it.
2. Open `.<version>.lock` and acquire its OS file lock; waiting for another
   installer or an uninstall is part of this step. Then inspect the destination.
   Reuse a complete entry without downloading; the lock guarantees only that the
   destination is not inspected or reused while an uninstall holds it. An
   uninstall that runs after install releases the lock can still remove the
   entry before the caller uses the path. Refuse an existing damaged entry
   without modifying it.
3. While holding that version's lock, remove only directories named
   `.tmp-<version>-<digits>`, with a nonempty ASCII decimal suffix. Preserve
   other versions, script stages, similarly prefixed names and non-directories.
4. Exclusively create `.tmp-<version>-<pid>` under `versions/`. Cleanup
   ownership starts only after creation succeeds. Download the archive and the
   checksum record and verify the checksum. Then download the signature file and
   verify the signature and its trusted comment. Extract only after both
   verifications succeed.
5. Accept exactly one regular root-level file for each required name: the target
   binary, `LICENSE`, and `THIRD-PARTY-NOTICES.txt`. Reject extra, nested,
   absolute, traversal, duplicate, symlink and hard-link entries, including an
   archive-provided `.complete`. Consume the gzip trailer so CRC errors and
   truncation cannot publish. Set the binary's Unix mode to `0755`.
6. Create the empty `.complete` marker locally. Refuse a destination that has
   appeared, then rename the stage to the final directory on the same
   filesystem. Keep the lock until publication or error cleanup finishes.
7. On failure, the stage guard removes only this operation's claimed stage. The
   lock file can remain; the OS lock is released when its handle closes.

The lock serializes CLI installers. It does not serialize the scripts or
external filesystem writers. If a destination appears at the CLI's final check,
the CLI returns an error and preserves it.

## Uninstall

`ridl toolchain uninstall <version>` removes one version directory, whether it
is complete or damaged. The root must be an absolute path: a relative
`RIDL_TOOLCHAIN_DIR` is an input error before any command runs.

1. Canonicalize `versions/`. An absent directory means the release is not
   installed; the command does not create it.
2. Refuse the running release: when the canonical running executable lies under
   the canonical entry, the command fails and leaves the entry unchanged. An
   entry that does not resolve skips this check.
3. Open `.<version>.lock` and acquire its OS file lock, as the CLI installer
   does, and remove the stale `.tmp-<version>-<digits>` stages under that lock.
4. Inspect the entry without following symbolic links. An absent entry means the
   release is not installed.
5. Rename the entry to `.tmp-<version>-<pid>`, so no reader sees a partly
   removed release, then remove the stage. A real directory is removed
   recursively; a symbolic link or a regular file is removed itself, and the
   target of a link is never followed. On Windows, when `remove_file` fails for
   a stage that is not a real directory, the stage is removed with `remove_dir`,
   which is how Windows removes a directory symbolic link or junction. That
   branch has no automated test.

Success prints the removed path under the canonical `versions/` directory. A
release that is not installed exits 1; every other failure exits 2. The lock
serializes CLI installers and uninstalls only. The install scripts take no lock,
so uninstall does not serialize against them.

## Install scripts and the PATH copy

`install.sh` and `install.ps1` use the same root and completion contract for
pinnable `ridl` releases. The
[getting-started instructions](../book/getting-started.md#installing-the-toolchain)
own version selection and environment settings. Scripts verify the checksum and
required archive files before publishing. They do not acquire the CLI lock.

The scripts accept decimal release components from 0 to 4294967295 (`u32`). The
CLI accepts components through 18446744073709551615 (`u64`), so a release
accepted by the CLI can exceed the scripts' component limit.

Their stage name is **`.tmp-script-<version>-<pid>`**, separate from CLI stages.
A script removes only a leftover stage with its own exact name before
exclusively creating and claiming it. PowerShell extracts directly into this
stage. Bash extracts into its `<version>/` child, so BSD and GNU `mv -n` can
publish to the `versions/` parent without nesting files inside an appeared
destination. Both create the marker locally before publication.

If another installer publishes first, the script reuses the appeared directory
only if it is complete, then removes its own stage. A damaged destination is
reported and preserved. A failed or skipped rename is followed by the same
completion check. Cleanup removes only the operation's claimed paths.

After reuse or publication, the script copies the binary through a temporary
regular file in the PATH destination directory, then renames it into place. The
versioned binary remains present; the PATH copy is not a symlink. Older `ridl`
releases, historical tags without a plain release and all `ridlc` installs
retain their flat PATH layout. This does not make `ridlc` a versioned inventory
entry.

## Regression evidence

The following tests cover the implemented contracts:

- `release_pin_accepts_only_three_decimal_components`,
  `release_compatibility_uses_major_minor_and_patch` and
  `release_refusal_precedes_members_overlays_and_remotes` in `ridl-core` cover
  parsing, comparison and exclusive refusal.
  `release_member_warning_at_deny_is_exit_one` in the CLI covers the member
  warning distinction.
- `release_refusal_is_published_on_manifest_value` and
  `release_refusal_replaces_analysis_and_recovers_from_manifest_buffer` in the
  LSP integration tests, and
  `release_refusal_is_reported_by_every_workspace_tool` in MCP, cover protocol
  refusals. The [book chapter](../book/release-pin.md#editors-and-agent-tools)
  describes their user-visible recovery.
- `toolchain_which_schema_one_covers_all_statuses` and
  `toolchain_which_selects_highest_compatible_including_running` in
  `crates/ridl/tests/toolchain.rs` cover the table above and tie preference.
  `toolchain_install_rejects_checksum_and_unsafe_archive`,
  `toolchain_install_waits_rechecks_and_preserves_damage` and
  `toolchain_install_removes_only_stale_cli_stages_under_lock` cover extraction
  and transaction boundaries. `toolchain_install_requires_a_valid_signature` and
  `toolchain_install_refuses_an_undecodable_public_key_before_download` cover
  the signature rules, and `signature_verifies_a_real_minisign_signature` and
  `signature_refuses_a_legacy_signature` in the `install.rs` unit tests pin
  compatibility with the `minisign` tool and the refusal of legacy signatures.
  The `release_signing` tests run the release workflow's signing step against a
  fake `minisign`. The ignored `toolchain_ureq_path_verifies_and_installs`
  exercises HTTP through `just install-check`.
- `toolchain_uninstall_removes_complete_and_damaged_entries`,
  `toolchain_uninstall_removes_a_symlink_entry_without_following_it`,
  `toolchain_uninstall_refuses_the_running_release`,
  `toolchain_uninstall_waits_for_the_version_lock` and
  `toolchain_uninstall_cli_streams_and_exit_codes` cover removal, and
  `toolchain_cli_refuses_a_relative_root` covers the absolute-root rule.
- `install_bare_and_tag_versions_share_layout`,
  `install_preserves_damaged_version`,
  `install_removes_only_its_own_stale_script_stage` and
  `install_reuses_complete_directory_appearing_at_rename` are fixture cases in
  `just install-check`.
- `schema_one_validates_every_state_and_nullability`,
  `missing_installs_requeries_then_starts`,
  `two_ridl_roots_inside_one_folder_get_two_clients`,
  `mcp_uses_first_folder_selected_binary` and
  `auto_install_is_application_scoped_and_trust_capability_is_absent` in the
  extension's fake-runner tests cover the consumer and host policy. Editor
  implementation ownership is in the
  [distribution record](../technotes/toolchain-distribution.md#editor-toolchain-selection).
