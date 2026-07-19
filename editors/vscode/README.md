# RIDL for VS Code

Editor support for `.typl` files: TextMate syntax highlighting and an LSP client
that connects to `ridl-lsp` (`crates/ridl-lsp`) for diagnostics and quick fixes.

This extension covers the `typl` language only. The other family languages
(`ridl`, `uxdl`, `rmdl`, `rsdl`) are sequenced separately in `docs/ROADMAP.md`.

## Prerequisites

`ridl-lsp` is a separate binary, built from this repository's Rust workspace.
Install it once:

```sh
cargo install --path crates/ridl-lsp
```

This places `ridl-lsp` in `~/.cargo/bin`, which most Rust installs already have
on `PATH`. If `ridl-lsp` is not on `PATH`, or a specific build should be used
instead (for example `target/debug/ridl-lsp` during development), set the
`ridl.serverPath` setting described below.

## Build the extension

From `editors/vscode`:

```sh
npm ci
npm run compile
npx @vscode/vsce package
```

This produces a `.vsix` file, for example `ridl-vscode-0.0.1.vsix`.

## Install

```sh
code --install-extension ridl-vscode-0.0.1.vsix
```

Open a folder containing `.typl` files. The extension activates on the `typl`
language, highlights the file, and starts `ridl-lsp` to publish diagnostics and
quick fixes.

Marketplace publishing is deferred to a maintainer act (like the crates.io
release) and is not part of this build.

## Settings

| Setting             | Default | Description                                                                                            |
| ------------------- | ------- | ------------------------------------------------------------------------------------------------------ |
| `ridl.serverPath`   | `""`    | Path to the `ridl-lsp` binary. Empty finds `ridl-lsp` on `PATH`.                                       |
| `ridl.trace.server` | `"off"` | Trace level for the JSON-RPC traffic between VS Code and `ridl-lsp` — `off`, `messages`, or `verbose`. |

## Development

```sh
npm run watch
```

Then use the VS Code "Run Extension" launch configuration (Extension Development
Host) against this folder, or open a `.typl` file in an instance of VS Code
started with `--extensionDevelopmentPath=editors/vscode`.
