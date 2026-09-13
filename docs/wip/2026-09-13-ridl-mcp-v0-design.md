# ridl-mcp v0 — a minimal MCP server — design

Date: 2026-09-13. Revised the same day after a design review; the review's
findings and their dispositions are folded in below.

## Purpose

ADR-0005 (agent enablement, status Proposed) specifies an MCP server — "Layer B
— Capability" — as a thin consumer of the shared compiler crates, sibling of the
LSP, exposing a six-tool minimum viable set (`ridl_check`, `ridl_explain`,
`ridl_diff`, `ridl_describe_type`, `ridl_list_interactions`, `ridl_resolve`). No
MCP code exists yet.

This design scopes the smallest real slice of that ADR: one tool, `ridl_check`,
shipped as a `ridl mcp` subcommand of the existing `ridl` CLI binary. It gives
the distribution pipeline
(`docs/wip/2026-09-13-vscode-extension-distribution-design.md`) a second entry
point to validate end to end — the extension must spawn the same binary in two
roles — before the remaining five tools are designed.

## Non-goals

- The other five tools. Each needs its own output-shape design; none is needed
  to validate the distribution pipeline.
- Workspace or multi-package resolution. `ridl_check` in this design checks one
  standalone source string, not a workspace member with external imports.
- Ratifying ADR-0005. It stays Proposed. This design implements one row of its
  table and resolves two of its open questions in passing (see "Decisions this
  design makes on ADR-0005's behalf"); it does not decide the others (eval
  scoring, skill generation, bridge exposure).

## Design

### 1. One binary, two roles: `ridl lsp` and `ridl mcp`

The `ridl` CLI (`crates/ridl`, "The RIDL toolchain": `check`, `baseline`,
`diff`, `fmt`, `test`, …) gains two subcommands:

- `ridl lsp` — runs the language server over stdio. `crates/ridl-lsp` is already
  a library (`ridl_lsp::server::run(connection)`) with a three-line `main.rs`;
  that binary target is removed and `ridl` depends on the library instead.
  `cargo install --path crates/ridl` replaces
  `cargo install --path crates/ridl-lsp` in the developer flow.
- `ridl mcp` — runs the MCP server over stdio, from a new library crate
  `crates/ridl-mcp` (its own git-std scope, added to `.git-std.toml`).

This is the shape `driftsys/markspec` actually ships (one binary, `lsp` and
`mcp` subcommands — `markspec/editors/vscode/DEVELOPMENT.md`), and it is what
lets one tarball serve every host: a Claude Code or Codex user who installs
`ridl` for MCP also gets `ridl check` and `ridl diff`, ADR-0005's CLI face
(Layer C), with no second artifact.

`wasm-check` does not build the `ridl` crate (its `Cargo.toml` records this), so
the MCP SDK's async runtime and `lsp-server` do not reach that gate.

### 2. Tool: `ridl_check(source, profile)`

Matches ADR-0005's table entry: given a source string, return the compiler's
structured diagnostics — coded, with spans and fix-its.

- **Input** — `source: string`, the full text of one file, and
  `profile: "typl" | "ridl"`, selecting which the compiler parses it as. These
  are the only two profiles the compiler has (`ridl_syntax::Profile`); the
  `.rxdl` single-file form ADR-0005 §7 names as the canonical agent target does
  not exist until epic E3.5 and is not accepted here. No workspace, no imports
  beyond the embedded `ridl.std`, which every profile already sees implicitly —
  the same "standalone file against `ridl.std`" path `ridl-lsp` already
  exercises for a buffer with no workspace (`crates/ridl-lsp/src/server.rs`, the
  single-file overlay).
- **Output** — a JSON diagnostic list. This is a **new, agent-facing contract**:
  no JSON diagnostic serialization exists in the toolchain today (`ridlc` emits
  IR JSON; `ridl` emits diff and property JSON; `ridl_core::Diagnostic` derives
  `Serialize`, but its `Span` carries a `FileId` and byte offsets that mean
  nothing to a caller who supplied a string). The contract, defined once in
  `ridl-core` and consumed by the tool:

  | Field      | Content                                                          |
  | ---------- | ---------------------------------------------------------------- |
  | `code`     | the catalogue code, for example `RIDL-107`                       |
  | `severity` | `error` / `warning` / `info`                                     |
  | `message`  | the primary message, verbatim                                    |
  | `span`     | 1-based `start`/`end` line and column, computed from `source`    |
  | `fixes`    | the fix-its, verbatim: a label and the replacement text per span |

  The stability of this shape is an agent-contract stability question (ADR-0005
  §7); it is the first such contract and is marked as such in the crate's
  README. `ridl check --format json` is added in the same pass, emitting the
  identical structure from the same `ridl-core` code, so the MCP tool has a CLI
  oracle to be compared against (the "byte-identical to CLI" property epic E8.6
  asks for has something to compare to).

### 3. Host coverage

Three hosts, all speaking MCP over stdio, reached by two different mechanisms:

- **GitHub Copilot** (inside VS Code) — served once the VS Code extension
  registers `ridl mcp` through `vscode.lm.registerMcpServerDefinitionProvider`
  (VS Code 1.101+), the mechanism `markspec-ide` already uses. Covered by the
  distribution design, not this one.
- **Claude Code** and **Codex** — standalone CLI agents that discover MCP
  servers from their own config (`.mcp.json` / `claude mcp add` for Claude Code;
  a `config.toml` `mcp_servers` entry for Codex), not from VS Code's extension
  registry. Both need only the `ridl` binary on `PATH` —
  `cargo install --path crates/ridl`, the release tarball via `install.sh` /
  `install.ps1`, or the extension's "Install to PATH" command.
  `crates/ridl-mcp`'s README carries a config snippet for each host, so the path
  is exercised rather than assumed.

## Decisions this design makes on ADR-0005's behalf

Recorded here so they are visible when ADR-0005 moves to Accepted, rather than
resolved silently:

- **"Does the same binary back a CLI entry?"** (ADR-0005 open question; parked
  roadmap block E8.8) — yes: `ridl lsp` and `ridl mcp` are subcommands of the
  `ridl` CLI.
- **"Which agent hosts are first-class?"** — Claude Code, GitHub Copilot, and
  Codex, all over stdio.
- **Build order.** ADR-0005 §1 orders knowledge first (skill and rules, epics
  E8.1–E8.3, not yet landed), capability second. This pass builds one capability
  tool before the knowledge layer, because the distribution pipeline needs a
  second entry point to validate. It does not reorder the epics: E8.1–E8.3
  remain ahead of the rest of E8, and `ridl_check` is the E8.6 tool, not the
  E8.5 skeleton.

## Testing

- Unit tests over the source-to-diagnostics path and the JSON contract, reusing
  the fixtures `ridl-sem`'s existing diagnostic tests already use — no new
  fixture corpus.
- One integration test that spawns the built `ridl` binary
  (`CARGO_BIN_EXE_ridl`) with `mcp`, speaks the MCP handshake over its stdio,
  calls `ridl_check`, and asserts the diagnostic list. This is new to the
  workspace: `crates/ridl-lsp/tests/server.rs` drives the server through
  `Connection::memory()`, never a real process, so there is no existing spawn
  harness to copy.
- One test that `ridl check --format json` and `ridl_check` agree on the same
  input.

## Related decisions

- ADR-0005 (agent enablement) — the six-tool table this design implements one
  row of, and the open questions it answers above.
- `docs/wip/2026-09-13-vscode-extension-distribution-design.md` — the bundling
  and release pipeline this subcommand is built to validate.
- `docs/ROADMAP.md` — epic E8.6 (`ridl_check`), parked block E8.8 (one binary),
  epic E3.5 (`.rxdl`).
