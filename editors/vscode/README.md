# RIDL for VS Code

Editor support for `.typl` and `.ridl` files: TextMate syntax highlighting and
an LSP client that connects to `ridl lsp` (`crates/ridl-lsp`) for diagnostics,
quick fixes, hovers, ordinal inlay hints, navigation, completion, and rename.

This extension covers the `typl` and `ridl` languages. One server serves both:
the compiler selects the profile from the file extension, so a `.ridl` interface
and the `.typl` vocabulary it imports are checked together. The remaining family
languages (`uxdl`, `rmdl`, `rsdl`) are sequenced separately in
`docs/ROADMAP.md`.

## Install

From the VS Code Marketplace or Open VSX, search "RIDL". The extension bundles
the `ridl` binary and runs it as `ridl lsp` (the language server) and `ridl mcp`
(the MCP server Copilot sees). Nothing else to install.

To use `ridl` from a terminal, Claude Code, or Codex as well, run the command
**RIDL: Install ridl to PATH**, or use the installer script from the repository
root (`install.sh` / `install.ps1`).

## Build from source

`ridl` on `PATH` is enough for a development launch (F5 in `editors/vscode`):

```sh
cargo install --path crates/ridl
```

To build a `.vsix` with the binary bundled:

```sh
just package-vscode
code --install-extension editors/vscode/ridl-vscode-<version>.vsix
```

## Usage

Open a folder containing `.typl` or `.ridl` files. The extension activates on
the `typl` and `ridl` languages, highlights the file, and starts `ridl lsp` to
publish diagnostics and quick fixes.

On a `.ridl` file the server additionally renders the ridl §11 ordinal beside
every interaction and `reserved` tombstone, expands an interaction's resolved
timing into the per-kind reading of family general form §6.2 on hover, and
offers the interaction keywords inside an interface body.

## Settings

| Setting             | Default | Description                                                                                                                              |
| ------------------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| `ridl.serverPath`   | `""`    | Path to the `ridl` binary. Empty (the default) uses the bundled binary, or `ridl` on `PATH` when none is bundled (a development launch). |
| `ridl.trace.server` | `"off"` | Trace level for the JSON-RPC traffic between VS Code and `ridl lsp` — `off`, `messages`, or `verbose`.                                   |

## Development

```sh
npm run watch
```

Then use the VS Code "Run Extension" launch configuration (Extension Development
Host) against this folder, or open a `.typl` or `.ridl` file in an instance of
VS Code started with `--extensionDevelopmentPath=editors/vscode`.
