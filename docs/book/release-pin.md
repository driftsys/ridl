# Pinning the workspace release

The [decisions](../decisions/ADR-0036-workspace-toolchain-pins.md) explain the
scope. The [toolchain directory record](../design/toolchain-directory.md) owns
the directory layout, selection schema and installation protocol.

A root `ridl.toml` can require a RIDL release. Set `ridl-version` in its
`[package]` or `[workspace]` table. This TOML fragment shows the key:

```toml
ridl-version = "0.23.0"
```

The value must be a string of three ASCII decimal numbers, `MAJOR.MINOR.PATCH`,
with each component fitting `u64` and no prefix, whitespace, pre-release suffix
or build suffix. Leading zeros are accepted and releases are compared
numerically. The minimum pin is 0.23.0. The running release must have the same
major and minor and a patch greater than or equal to the pin. For example,
0.23.2 accepts a 0.23.0 pin; 0.22.9, 0.24.0 and 1.23.0 do not.

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
`describe` and a `diff` directory input report a root release refusal before an
I/O error of the input. Explicit snapshot inputs to `diff`, standalone
descriptors, `init`, `new` and `lock merge` keep their existing contracts,
independent of the current directory's pin. `lsp` and `mcp` still start their
servers and report refusals through their protocols, as described below.

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
the platform data directory below. `ridl` refuses a relative override, exit 2.
Empty environment values count as unset.

| Platform | Default root                                                                |
| -------- | --------------------------------------------------------------------------- |
| Linux    | `$XDG_DATA_HOME/ridl`, or `~/.local/share/ridl` when XDG_DATA_HOME is unset |
| macOS    | `~/Library/Application Support/ridl`                                        |
| Windows  | `%LOCALAPPDATA%\ridl`                                                       |

A missing home directory or required Windows variable is an error. Releases
remain side by side; an existing damaged installation is reported and preserved.
The [directory and installation protocol](../design/toolchain-directory.md)
defines completion, archive verification, locking, separate CLI/script staging
and cleanup ownership.

`ridl toolchain uninstall VERSION` removes the version directory of an exact
release of at least 0.23.0, complete or damaged, and prints the removed path. It
refuses the running release, exits 1 when the release is not installed, and
waits for the version's lock. The
[directory record](../design/toolchain-directory.md#uninstall) defines the
steps.

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
stderr for a negative answer. `--json` provides the
[schema-1 selection report](../design/toolchain-directory.md#which---json-schema-1),
including all status, nullability and exit mappings. Selecting a binary does not
run it; invoke the returned path explicitly in terminal workflows.

## Editors and agent tools

The VS Code extension selects a release for each discovered RIDL root. It runs
`ridl toolchain which --json <document directory>` through its bundled binary,
or through `ridl` on PATH in a development launch without a bundled binary. It
starts the validated result directly as `ridl lsp`, without a proxy. Compatible
and unpinned results use the selected path. Invalid pins use the resolver binary
so that its server reports the manifest diagnostic.

When a valid pin is missing, `ridl.toolchain.autoInstall` controls installation.
This boolean defaults to `true` and has application scope: workspace settings
cannot change it. Automatic installation runs `ridl toolchain install <pin>`
under a progress notification, then queries selection again. Concurrent roots
requiring the same release share the installation. With automatic installation
disabled, the resolver server starts immediately and a notification offers an
**Install** button. Accepting it installs and selects that release.

An installation failure keeps the resolver server, which reports MANI-026 when
its release is incompatible. The extension reports the installation error once
per release during the activation; a later open can retry. An unsupported JSON
schema keeps the resolver binary and logs a warning without an error
notification. A malformed response, unsupported toolchain command or failed
discovery also keeps the resolver binary and logs a warning; its error
notification is shown once during the activation. A successful installation with
no usable selection is treated as a failure, without an installation loop.

`ridl.serverPath` is an explicit developer override, read for the applicable VS
Code folder. It bypasses discovery and installation and uses that folder as the
server boundary. Its server still enforces pins. Otherwise, the root manifest
returned by discovery defines the boundary, including two RIDL roots inside one
VS Code folder and documents inside workspace members. Manifestless documents
share a separate client. Temporary folder boundaries used after a failed probe
are replaced when valid discovery succeeds. Routing prevents a parent or
manifestless client from handling a nested root's document traffic, diagnostics
or edits.

A RIDL source activates the extension through its language contribution. A
`ridl.toml` found inside an opened VS Code folder activates it through
`workspaceContains:**/ridl.toml`; opening a manifest alone in a window without a
folder does not activate it. Once the extension is active, opening a RIDL source
or a manifest starts its client. The manifest is selected by filename in any
language mode, so the pin action works before a source is opened and without a
separate TOML extension. Other TOML files do not start clients. Saved manifest
create/change/delete events are forwarded to the servers and run discovery
again. A client is replaced when its selected binary or discovery key changes,
including when a manifestless document acquires a root manifest. Unsaved
manifest changes reload the current server without selecting a release from
unsaved text. Removing a VS Code folder stops clients no longer used by another
mapping, and deactivation stops every client.

The extension declares no support for untrusted workspaces. VS Code therefore
keeps it inactive in Restricted Mode: no server or installer runs. Granting
Workspace Trust enables normal activation. The MCP definition uses the release
selected for the first VS Code workspace folder, independently of document-open
order. There is one MCP definition per window; a selection change updates it.

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
clears the refusal without restarting the server. Member manifest events do not
reload the workspace, replace the root or become source overlays.

On an unpinned root manifest, the server offers the `refactor.rewrite` code
action **Pin this workspace to ridl &lt;running release&gt;**. It inserts the
key without rewriting existing bytes or line endings. It supports explicit
tables, inline tables and dotted keys. It requires valid TOML with exactly one
`[package]` or `[workspace]` mode and a running release of at least 0.23.0. An
existing key, including an invalid value, suppresses the action. The action
needs no diagnostic and respects the client's requested code action kinds.

MCP path-mode `ridl_check` returns the root refusal diagnostic, even for a
member path. Metrics and the five name/dependency lookup tools return a tool
error containing the refusal code and message. Source-side `ridl_diff` retains
its diagnostic-bearing error result. Root manifest overlays are checked before
source overlays. Source-text-only `ridl_check`, `ridl_explain` and snapshot diff
inputs remain independent of workspace pins. The
[MCP contract](../../crates/ridl-mcp/README.md#tools) records the schema
structure and overlay description changes.
