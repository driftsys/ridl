# RIDL for VS Code

Editor support for the RIDL family: `typl`, the type language; `ridl`, the
interface description language; and `rsdl`, the system description language. One
language server serves all three, since the compiler checks a `.rsdl` system,
the `.ridl` services it names, and the `.typl` vocabulary they import together.
The remaining family members (`rmdl`, and the `rxdl` file kind) are sequenced
separately in
[`docs/ROADMAP.md`](https://github.com/driftsys/ridl/blob/main/docs/ROADMAP.md).

Learn the languages:
[getting started](https://driftsys.github.io/ridl/getting-started.html),
[typl reference](https://driftsys.github.io/ridl/reference/typl.html),
[ridl reference](https://driftsys.github.io/ridl/reference/ridl.html),
[rsdl reference](https://driftsys.github.io/ridl/reference/rsdl.html).

## Features

- **Syntax highlighting** for `.typl`, `.ridl` and `.rsdl` files.
- **Language server** (`ridl lsp`): diagnostics, quick fixes, hovers, ordinal
  inlay hints, navigation, completion, rename, and whole-document formatting
  with the `ridl fmt` style, across all three languages. The
  [workspace release pin chapter](https://driftsys.github.io/ridl/release-pin.html#editors-and-agent-tools)
  describes pinned release selection, automatic installation and the Install
  button, fallback errors, explicit overrides, Workspace Trust, multiple roots
  and the first-folder MCP limitation.
- **On a `.ridl` file**: the ridl §11 ordinal renders beside every interaction
  and `reserved` tombstone, hovering an interaction expands its resolved timing
  into the per-kind reading of family general form §6.2, and completion offers
  the interaction keywords inside an interface body.
- **On a `.rsdl` file**: the system checks of the rsdl reference publish over
  the whole workspace; hover, go-to-definition and find-references follow a
  reference to the component, instance, system, service or interface it names;
  rename rewrites a system, component or instance with every reference to it;
  and completion offers what each slot admits — a service after `offers`, an
  interface after `requires`, a component on a member line.
- **MCP server** (`ridl mcp`): the extension registers an MCP server definition,
  so an MCP-aware agent host (Claude Code, GitHub Copilot, or another MCP
  client) can call the same `ridl_check` tool — the same parser, resolver and
  checker the editor and `ridl check` use — against a source string for `typl`,
  `ridl` or `rsdl`.
- **RIDL: Install ridl to PATH** command: copies the bundled `ridl` binary onto
  your terminal `PATH`, for use outside the editor.

## Install

Releases are published to the VS Code Marketplace under "RIDL". A maintainer
cuts each one from an `editor-v*` tag. The published extension bundles the
`ridl` binary used to select and install workspace toolchains. Selected binaries
run the language server and the MCP server Copilot sees. Before the first
release, or to test a change, build the extension from source (below). The
repository root also has installer scripts (`install.sh` / `install.ps1`) that
download `ridl` alone from the newest `editor-v*` GitHub Release.

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
code --install-extension editors/vscode/ridl-lang-<version>.vsix
```

## Settings

| Setting                      | Default | Description                                                                                                                   |
| ---------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `ridl.serverPath`            | `""`    | Explicit path to the `ridl` binary; bypasses workspace toolchain selection. Empty uses the release selection described above. |
| `ridl.toolchain.autoInstall` | `true`  | Automatically install a missing workspace release. Application setting; workspace settings cannot change it.                  |
| `ridl.trace.server`          | `"off"` | Trace level for JSON-RPC traffic between VS Code and `ridl lsp`: `off`, `messages`, or `verbose`.                             |

If `ridl.serverPath` names the retired `ridl-lsp` binary, the extension warns;
set it to the `ridl` binary, which runs as `ridl lsp`.

## Development

```sh
npm run watch
```

Then use the VS Code "Run Extension" launch configuration (Extension Development
Host) against this folder, or open a `.typl` or `.ridl` file in an instance of
VS Code started with `--extensionDevelopmentPath=editors/vscode`.
