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
an installed candidate would be needed there, it returns an error instead.
Binary naming follows the supplied target, including `ridl.exe` for Windows.

Production requests use
`https://github.com/driftsys/ridl/releases/download/editor-v<version>/ridl-<target>.tar.gz`
and the same URL with `.sha256` appended. The library's `base_url` parameter
allows fixture `file://` and local HTTP sources; the CLI always passes its
constant origin and exposes no override. Each HTTP request has a 30-second
connect timeout and a 10-minute overall timeout. Downloads are bounded to 256
MiB for the archive and 4 KiB for the checksum record.

The checksum record is one UTF-8 line: 64 hexadecimal SHA-256 digits, then two
spaces or a space followed by `*`, then the exact archive name, with an optional
LF or CRLF. The computed digest must match, case-insensitively. This verifies
consistency with the origin's record, not an independently signed publisher
identity.

## CLI installation transaction

An explicit install request parses the requested release and checks the pin
floor. An omitted version reads the current root pin; absent or invalid pins are
input errors. Neither form compiles sources or enforces compatibility with the
resolver. Success prints the canonical absolute installed binary path. Failures
return exit 2.

1. Validate release and target, create `versions/` and canonicalize it. Inspect
   the destination. Reuse a complete entry immediately, without downloading;
   refuse an existing damaged entry without modifying it.
2. Open `.<version>.lock` and acquire its OS file lock. Recheck the destination
   after waiting: another cooperating installer may have completed it.
3. While holding that version's lock, remove only directories named
   `.tmp-<version>-<digits>`, with a nonempty ASCII decimal suffix. Preserve
   other versions, script stages, similarly prefixed names and non-directories.
4. Exclusively create `.tmp-<version>-<pid>` under `versions/`. Cleanup
   ownership starts only after creation succeeds. Download and verify both
   assets before extracting.
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
   target of a link is never followed.

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
  and transaction boundaries. The ignored
  `toolchain_ureq_path_verifies_and_installs` exercises HTTP through
  `just install-check`.
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
