# Pinning the workspace release

A root `ridl.toml` can require a RIDL release. Set `ridl-version` in its
`[package]` or `[workspace]` table. This TOML fragment shows the key:

```toml
ridl-version = "0.23.0"
```

The value must be a string of three ASCII decimal numbers, `MAJOR.MINOR.PATCH`,
with no prefix, whitespace, pre-release suffix or build suffix. Leading zeros
are accepted and releases are compared numerically. The minimum pin is 0.23.0.
The running release must have the same major and minor and a patch greater than
or equal to the pin. For example, 0.23.2 accepts a 0.23.0 pin; 0.22.9, 0.24.0
and 1.23.0 do not.

Only the root manifest returned by workspace discovery pins the release. A path
inside a workspace member still uses the workspace root. A member's key is
ignored and draws `member-ridl-version` (MANI-027), even when its value is
invalid. Remote unit pins do not constrain the consumer. Without a root pin,
commands retain their existing behavior.

| Code     | Meaning                                                 |
| -------- | ------------------------------------------------------- |
| MANI-024 | The pin is not a string of the required form.           |
| MANI-025 | The pin is below 0.23.0.                                |
| MANI-026 | The running release is incompatible with the pin.       |
| MANI-027 | A member manifest's pin is ignored; warning by default. |

The diagnostic points to the value. MANI-026 names both releases and the command
`ridl toolchain install <pin>` for obtaining the required release.

MANI-024 through MANI-026 stop loading before members, source parsing, headers,
remote imports, lockfiles or lint directives. They are the only diagnostics
reported for that root. The batch commands listed below exit **2** without
writing workspace or output files. Invalid TOML syntax remains MANI-001.
MANI-027 raised to `deny` is an ordinary diagnostic error and exits **1** in
commands that apply levels.

`check`, `build`, `fmt`, `lock`, source inputs to `diff`, `baseline`, `test`,
`vendor`, `describe`, and `ridlc check/build` enforce the root pin. Formatting
checks every discovered root before any write, without compiling sources.
`describe` checks the descriptor input's root without compiling sources.
Explicit snapshot inputs to `diff`, standalone descriptors, `init`, `new` and
`lock merge` keep their existing contracts, independent of the current
directory's pin. `lsp` and `mcp` still start their servers and report refusals
through their protocols, as described below.

Releases before 0.23.0 cannot enforce pins: they read the key as unknown
(MANI-005) and may continue. Pin support begins with 0.23.0.

The
[toolchain distribution record](../technotes/toolchain-distribution.md#how-the-binary-learns-its-version)
defines build-version normalization and the release used by pin checks.

## Installing and selecting toolchains

`ridl toolchain install [VERSION]` installs an exact release of at least 0.23.0.
Without `VERSION`, it reads the root pin above the current directory. No pin, an
invalid pin or invalid TOML is an input error, exit 2. This command reads the
pin without enforcing compatibility with the running binary.

The toolchain root is the nonempty `RIDL_TOOLCHAIN_DIR` environment variable, or
the platform data directory below. A relative override is resolved against the
current directory. Empty environment values count as unset.

| Platform | Default root                                                                |
| -------- | --------------------------------------------------------------------------- |
| Linux    | `$XDG_DATA_HOME/ridl`, or `~/.local/share/ridl` when XDG_DATA_HOME is unset |
| macOS    | `~/Library/Application Support/ridl`                                        |
| Windows  | `%LOCALAPPDATA%\ridl`                                                       |

A missing home directory or required Windows variable is an error. Each release
lives in `<root>/versions/<MAJOR.MINOR.PATCH>/`, containing `ridl` (`ridl.exe`
on Windows), `LICENSE`, `THIRD-PARTY-NOTICES.txt` and an empty `.complete` file.
A missing binary or missing, non-file or nonempty marker makes the directory
damaged. Installation reports that path and preserves it; it does not repair or
delete existing directories.

Installation uses the running binary's build target to download an archive and
its SHA-256 record from the fixed GitHub editor release origin. The record must
name that archive and its digest must match. Only the three root-level regular
files above are accepted; links, nested paths, duplicates and archive-provided
completion markers are refused. A per-version file lock serializes installers.
The installer rechecks under the lock and removes stale temporary directories
for that version before staging. It extracts into its own temporary directory,
then creates the marker and renames the directory into place. Failures remove
that operation's temporary directory. Downloads have a 30-second connection
timeout and a 10-minute overall timeout per request. Archives larger than 256
MiB and checksum records larger than 4 KiB are refused. An existing complete
release works offline. Unsupported release targets produce a named error.

`ridl toolchain list` prints one `<version>\t<directory>` line per canonical
version directory, in numeric order. Damaged lines end with `(damaged)`.
Temporary directories, lock files and unrelated names are ignored. An empty
inventory exits 0; read errors exit 2.

`ridl toolchain which [PATH] [--json]` defaults to the current directory. It
selects the highest compatible complete installed release or the running binary,
using the running binary on an equal-version tie. It never downloads. Unpinned
workspaces use the running binary. Compatible running binaries remain usable on
targets without published archives.

Text output is the absolute binary path on stdout for success, or a sentence on
stderr for a negative answer. `--json` writes a schema-1 object with eight keys:
`schema`, `manifest`, `pin`, `running`, `status`, `path`, `source`,
`diagnostic`. Manifest and binary paths are canonical absolute paths. Absent
values are null.

| Status       | Exit | Path and source                           | Diagnostic                                 |
| ------------ | ---- | ----------------------------------------- | ------------------------------------------ |
| `compatible` | 0    | selected binary; `installed` or `running` | null                                       |
| `unpinned`   | 0    | running binary; `running`                 | null                                       |
| `missing`    | 1    | null                                      | null                                       |
| `invalid`    | 1    | null                                      | `code` and `message`, MANI-024 or MANI-025 |

An invalid string has a null pin. A parsed below-floor pin retains its canonical
string. No manifest means a null manifest; an unpinned manifest remains present.
A missing entry, unreadable manifest, invalid TOML or wrong-type mode table
exits 2, with no successful JSON on stdout. These management commands do not
enforce the running binary's pin compatibility and never report MANI-026 as an
invalid selection answer. Selecting a binary does not run it; invoke the
returned path explicitly in terminal workflows.

## Editors and agent tools

The language server publishes MANI-024, MANI-025 or MANI-026 on the root
`ridl.toml` value. It stays running and clears earlier source diagnostics, quick
fixes and navigation results. Source analysis, completion, formatting and rename
remain unavailable while the root is refused, including for newly opened
sources. Open source text is retained for recovery.

Opening, changing or closing the root manifest buffer reloads the workspace.
`workspace/didChangeWatchedFiles` create/change/delete events for that manifest
also reload it. An open root buffer takes precedence over disk text only while
the manifest exists on disk. Deleting the disk manifest unloads the workspace
and clears its diagnostics; edits to its open buffer do not load it again. A
watched create event after the file is restored, including by saving it, reloads
the workspace with the current buffer text. Fixing the buffer or the saved pin
clears the refusal without restarting the server. Member manifest buffers do not
replace the root or become source overlays.

On an unpinned root manifest, the server offers the `refactor.rewrite` code
action **Pin this workspace to ridl <running release>**. It inserts the key
without rewriting existing bytes or line endings. It supports explicit tables,
inline tables and dotted keys. It requires valid TOML with exactly one
`[package]` or `[workspace]` mode and a running release of at least 0.23.0. An
existing key, including an invalid value, suppresses the action. The action
needs no diagnostic and respects the client's requested code action kinds.

MCP path-mode `ridl_check` returns the root refusal diagnostic, even for a
member path. Metrics and the five name/dependency lookup tools return a tool
error containing the refusal code and message. Source-side `ridl_diff` retains
its diagnostic-bearing error result. Root manifest overlays are checked before
source overlays. Source-text-only `ridl_check`, `ridl_explain` and snapshot diff
inputs remain independent of workspace pins. No tool name or schema changes.
