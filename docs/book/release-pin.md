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
`ridl toolchain install <pin>` for obtaining the required release. Toolchain
management is tracked in
[driftsys/ridl#1476](https://github.com/driftsys/ridl/issues/1476).

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

## Editors and agent tools

The language server publishes MANI-024, MANI-025 or MANI-026 on the root
`ridl.toml` value. It stays running and clears earlier source diagnostics, quick
fixes and navigation results. Source analysis, completion, formatting and rename
remain unavailable while the root is refused, including for newly opened
sources. Open source text is retained for recovery.

Opening, changing or closing the root manifest buffer reloads the workspace.
`workspace/didChangeWatchedFiles` create/change/delete events for that manifest
also reload it; an open root buffer takes precedence over disk text. Fixing the
buffer or the saved pin clears the refusal without restarting the server. Member
manifest buffers do not replace the root or become source overlays.

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
