# RIDL for VS Code

Editor support for `.typl`, `.ridl` and `.rsdl` files: TextMate syntax
highlighting and an LSP client that connects to `ridl lsp` (`crates/ridl-lsp`)
for diagnostics, quick fixes, hovers, ordinal inlay hints, navigation,
completion, and rename.

This extension covers the `typl`, `ridl` and `rsdl` languages. One server serves
the three: the compiler selects the profile from the file extension, so a
`.rsdl` system, the `.ridl` services it names and the `.typl` vocabulary they
import are checked together. The remaining family languages (`rxdl`, `rmdl`) are
sequenced separately in `docs/ROADMAP.md`.

## Install

Releases are published to the VS Code Marketplace and Open VSX under "RIDL". A
maintainer cuts each one from an `editor-v*` tag. The published extension
bundles the `ridl` binary and runs it as `ridl lsp` (the language server) and
`ridl mcp` (the MCP server Copilot sees). Before the first release, or to test a
change, build the extension from source (below). The repository root also has
installer scripts (`install.sh` / `install.ps1`) that download `ridl` alone from
the newest `editor-v*` GitHub Release.

Once the extension is installed, by whichever route, run the command **RIDL:
Install ridl to PATH** to use `ridl` from a terminal, Claude Code, or Codex too.

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

Open a folder containing `.typl`, `.ridl` or `.rsdl` files. The extension
activates on the `typl`, `ridl` and `rsdl` languages, highlights the file, and
starts `ridl lsp` to publish diagnostics and quick fixes.

On a `.ridl` file the server additionally renders the ridl §11 ordinal beside
every interaction and `reserved` tombstone, expands an interaction's resolved
timing into the per-kind reading of family general form §6.2 on hover, and
offers the interaction keywords inside an interface body.

On a `.rsdl` file the server publishes the system checks of the rsdl reference
over the whole workspace, and hover and go-to-definition follow a reference to
the component, instance, system, service or interface it names.

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
