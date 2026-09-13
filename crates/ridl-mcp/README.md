# ridl-mcp

The RIDL MCP server (ADR-0005 Layer B): `ridl mcp` serves the Model Context
Protocol over stdio with one tool, `ridl_check`. It is a thin consumer of the
shared compiler crates — the same parser, resolver, and checker `ridl check` and
`ridl lsp` use.

## Install

`ridl` must be on `PATH`:

```sh
cargo install --path crates/ridl
```

Installer scripts, `install.sh` and `install.ps1`, exist at the repository root
and download the binary from a GitHub Release; a maintainer has not yet
published one.

## Tools

### `ridl_check`

| Parameter | Type                 | Meaning                              |
| --------- | -------------------- | ------------------------------------ |
| `source`  | string               | the full text of one file            |
| `profile` | `"typl"` or `"ridl"` | which language `source` is parsed as |

Returns `{ "diagnostics": [ … ] }`, where each element is:

| Field      | Content                                                                      |
| ---------- | ---------------------------------------------------------------------------- |
| `code`     | the catalogue code, for example `RIDL-107`                                   |
| `severity` | `error`, `warning`, or `info`                                                |
| `message`  | the primary message, verbatim                                                |
| `span`     | `path`, and 1-based `start`/`end` (`end` exclusive) with `line` and `column` |
| `fixes`    | fix-its, verbatim: `label`, `replacement`, `span`                            |

**What this tool shares with `ridl check --format json <file>`, and where it
differs.** Both call the same `ridl_core::diag::to_json`, so each diagnostic
object listed above is identical between the two faces. Two things are not:

- **The top level.** This tool returns the object `{"diagnostics": [ … ]}`.
  `ridl check --format json` prints the bare array instead, with no
  `diagnostics` key.
- **`span.path`.** This tool's input is a source string with no file, so every
  span reports the fixed synthetic path `input.typl` or `input.ridl`.
  `ridl check --format json <file>` reports the real path it read.

This is the first agent-facing diagnostic contract; a change to its shape is a
change to an external contract (ADR-0005 §7).

The source is checked as a standalone file against the embedded `ridl.std` only:
no workspace, no other imports. The `.rxdl` form is not a profile yet (epic
E3.5).

## Host configuration

### Claude Code

Add to the project's `.mcp.json` (or run `claude mcp add ridl -- ridl mcp`):

```json
{
  "mcpServers": {
    "ridl": { "command": "ridl", "args": ["mcp"] }
  }
}
```

### Codex

Add to `~/.codex/config.toml`:

```toml
[mcp_servers.ridl]
command = "ridl"
args = ["mcp"]
```

### GitHub Copilot in VS Code

Installing the RIDL VS Code extension registers `ridl mcp` with VS Code
automatically, on VS Code 1.101 or newer, through
`vscode.lm.registerMcpServerDefinitionProvider`. No manual configuration is
needed.

A user who is not using the extension configures it manually instead: install
`ridl` on `PATH` as above, then add it as a server the same way the Claude Code
or Codex sections above do, in whichever place your version of VS Code keeps its
MCP server list.
