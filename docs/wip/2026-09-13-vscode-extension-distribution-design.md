# VS Code extension and toolchain distribution — design

Date: 2026-09-13. Revised the same day after a design review; the review's
findings and their dispositions are folded in below.

## Purpose

`editors/vscode` and `crates/ridl-lsp` are functional (landed under Epic 1 as
E1.15, E1.16, E1.17 — `docs/archive/roadmap-landed-record.md`), but the
extension is installed today only by building a `.vsix` locally and running
`code --install-extension`, and the language server only by `cargo install`.
This design adds a repeatable distribution path for the `ridl` toolchain binary
and the extension: a GitHub Release carrying per-platform `.vsix` files and
plain binary tarballs, a publish path to the VS Code Marketplace and Open VSX,
and script/command install paths for users who never install VS Code.

The extension bundles one binary, `ridl`, and spawns it in two roles —
`ridl lsp` and `ridl mcp` (`docs/wip/2026-09-13-ridl-mcp-v0-design.md`) — so the
pipeline is proven with both entry points before more MCP tools are added.

## Non-goals

- The full six-tool MCP surface ADR-0005 specifies. Only the `ridl mcp`
  subcommand with `ridl_check` is bundled here.
- Changing how the Rust workspace's crates are versioned on crates.io. The
  workspace crates stay at `0.0.0` per ADR-0007 decision 14 until a maintainer
  decides to cut a real preview. This design does open a second, public release
  train for the `ridl` binary — see "Versioning" — and says so rather than
  letting it happen quietly.
- Publishing anything as part of this design's own implementation. Every step
  that reaches an external registry (a tag push, a Marketplace publish approval)
  stays a deliberate maintainer act, per ADR-0007 decision 14.
- Build provenance attestation for the release artifacts. A follow-up.

## Precedent

`driftsys/markspec` already ships its VS Code extension (`markspec-ide`) through
the same publisher (`driftsys`) to both the VS Code Marketplace and Open VSX,
with one bundled binary spawned as `markspec lsp` / `markspec mcp`, an
environment-gated publish step, plain-tarball releases, `install.sh` and
`install.ps1`, and an "install to PATH" command. This design ports that pattern.
Two deliberate differences:

1. RIDL's extension keeps a `PATH` fallback so an unpackaged
   extension-development launch works after
   `cargo install --path
   crates/ridl` with no setting; markspec's resolver
   has no such tier.
2. Gate commands are defined in the justfile and invoked from the workflows
   (ADR-0009); markspec writes them inline.

## Design

### 1. Verify workflow

New `.github/workflows/vscode-verify.yaml`, triggered on pull requests that
touch `editors/vscode/**`, the two new workflows, or the justfile. It runs one
recipe, `just vscode-verify`:

- `npm ci` and `npm run compile` in `editors/vscode`
- `npx vsce package --out /tmp/ridl-vscode-verify.vsix` — validates the
  manifest, icon, and gallery metadata with no credentials and **no bundled
  binary**: a fresh checkout has no `bin/`, and the manifest does not reference
  it, so packaging succeeds.

This closes a real gap: RIDL's CI does not currently build or check
`editors/vscode` at all. `just gate-parity` reads only `ci.yml`, so it does not
cover this workflow; the recipe is not a member of `just build`, and that is
stated in the justfile comment.

### 2. Binary resolution (extension code change)

`editors/vscode/src/extension.ts` currently resolves the server command as
"`ridl.serverPath`, else `ridl-lsp` on `PATH`". This design extracts the
resolution into one pure, unit-testable module,
`editors/vscode/src/binaryResolution.ts` (after markspec's
`src/serverOptions.ts`), resolving the single `ridl` binary once and spawning it
with `["lsp"]` or `["mcp"]`:

1. `ridl.serverPath`, if set — the developer override. Its meaning changes from
   "path to `ridl-lsp`" to "path to `ridl`"; the setting description and the
   README say so, and the extension shows one warning when the configured file
   is named `ridl-lsp`.
2. The bundled binary at `<extensionPath>/bin/ridl` (`.exe` on Windows), if that
   file exists — every Marketplace, Open VSX, or `.vsix` install.
3. The bare command `ridl`, resolved from `PATH` — reached only when `bin/` is
   absent, which is the unpackaged extension-development launch. A packaged
   install never reaches this tier: the bundled binary shadows a
   `cargo install`ed one unless tier 1 is set.

### 3. MCP registration (extension code change, manifest change)

The extension registers `ridl mcp` through
`vscode.lm.registerMcpServerDefinitionProvider` with a
`McpStdioServerDefinition`, resolved through the module above, and fires the
provider's change event when `ridl.serverPath` changes — the same reaction the
LSP client already has. This serves **GitHub Copilot** inside VS Code. **Claude
Code** and **Codex** do not read VS Code's registry; they need `ridl` on `PATH`
(sections 5 and 6) and the config snippets in `crates/ridl-mcp`'s README.

Two manifest changes follow, and one consequence is a decision:

- `engines.vscode` and `@types/vscode` move from `^1.91.0` to `^1.101.0` (`vsce`
  refuses a manifest whose `@types/vscode` is newer than its `engines.vscode`),
  and `contributes.mcpServerDefinitionProviders` declares the provider.
- **Consequence:** a VS Code fork below 1.101 can no longer install the
  extension at all — including the LSP it could use today. ADR-0005 names Cursor
  as a host. Decision: accept the bump. The alternative — keep `engines` at
  `^1.91.0`, hand-declare the MCP API types, and guard the registration at
  runtime — carries an unversioned copy of a VS Code API in this repository for
  a fork that tracks upstream within months. If a user on an older fork reports
  the block, this decision is the one to revisit.

### 4. Release workflow

New `.github/workflows/vscode-release.yaml`, triggered by a push of a tag
matching `editor-v*` (see "Versioning" below):

- **`guard`** — fails fast unless the tag's version equals
  `editors/vscode/package.json`'s `version`. `vsce publish` takes the version
  from the manifest, so a mismatch would publish one version under another's
  tag.
- **`build`** — a matrix over five targets (`x86_64-unknown-linux-gnu`,
  `aarch64-unknown-linux-gnu`, `x86_64-apple-darwin`, `aarch64-apple-darwin`,
  `x86_64-pc-windows-msvc`). Each cell runs
  `cargo build --release --locked --target <triple> -p ridl` with
  `RIDL_BUILD_VERSION=<tag>` in the environment (see "Versioning"), then tars
  the binary with a `sha256` checksum file (`ridl-<target>.tar.gz`, `.sha256`)
  and uploads both as artifacts.
- **`package-vsix`** — for each target, downloads the binary, copies it to
  `editors/vscode/bin/ridl` (`.exe` on Windows), then runs
  `npx vsce package --target <vsce-target> --out ridl-vscode-<vsce-target>.vsix`.
  Five `.vsix` artifacts; the publish jobs check for exactly five.
- **`release`** — downloads every tarball, checksum, and `.vsix` and creates the
  GitHub Release (`softprops/action-gh-release`) with generated release notes
  and `make_latest: false`, so this train never becomes the repository's
  "latest" release (see `install.sh`). This job does not depend on the publish
  jobs, so a Marketplace or Open VSX failure never blocks the release itself.
- **`publish-vsce`** and **`publish-ovsx`** — each gated by
  `environment: marketplace`, which GitHub holds for a maintainer's manual
  approval (the environment must be configured with required reviewers, matching
  markspec). Each downloads the five `.vsix` files and runs `vsce publish` /
  `ovsx publish` on each, using `secrets.VSCE_PAT` / `secrets.OVSX_PAT`.

Neither `vsce publish` nor `ovsx publish` is idempotent: re-running a publish
job after a partial failure re-attempts every platform and fails on the first
one already published. A partial failure is resolved by publishing the remaining
platforms individually, not by re-running the job.

### 5. `install.sh` and `install.ps1`

Two new top-level scripts, following the convention this repo's own CI already
trusts for `prim` (`curl -fsSL …/install.sh | bash`,
`.github/workflows/ci.yml`), and markspec's pair. Each: detects OS and
architecture, resolves the newest `editor-v*` release **by listing releases and
filtering on the tag prefix** — never `/releases/latest`, which returns the
newest release across every tag and would pick a workspace `v*` release once one
exists — or takes a version argument, downloads `ridl-<target>.tar.gz`, verifies
the `sha256`, and installs `ridl` to a `PATH` directory (or an `--install-dir`
override). The PowerShell script updates the user `PATH` through the registry,
not `setx`, which truncates values over 1024 characters.

### 6. "Install to PATH" command (extension code change)

Ported from markspec's `installToPath.ts`: a command `ridl.installToPath` that
copies the bundled `bin/ridl` to a `PATH` directory, checks whether that
directory is on `PATH`, and shows a hint when it is not. Because the extension
auto-updates and a copy on `PATH` does not, the command records nothing but the
binary itself: on activation the extension runs `ridl --version` on the bundled
binary and on the `PATH` copy (if one exists in the directory it installs to)
and shows one information message offering to re-copy when they differ. This is
what makes the copy's staleness detectable at all — it depends on the injected
version below.

### 7. `just` recipes

- `just vscode-verify` — section 1. No binary.
- `just package-vscode` — builds `ridl` for the host platform
  (`cargo build --release --locked -p ridl`), copies it into
  `editors/vscode/bin/`, and runs `vsce package`. Local testing only; the
  release workflow's per-target steps are the same commands with a `--target`,
  so a change to the packaging command is made in the recipe and mirrored in the
  workflow's one matrix step.

Both are defined in the justfile; the workflows invoke recipes (ADR-0009).

### 8. Repository hygiene

- `editors/vscode/bin/` is gitignored: `just package-vscode` writes a
  multi-megabyte binary there.
- `editors/vscode/.vscodeignore` is added — `vsce` reads only that file to
  decide what a `.vsix` contains, and none exists today, so `src/`,
  `tsconfig.json`, and `node_modules` would ship.
- `.git-std.toml` gains the `ridl-mcp` scope.
- The `ridl-lsp` binary target is removed (the crate stays a library);
  `editors/vscode/README.md` and `CONTRIBUTING.md` replace
  `cargo install --path crates/ridl-lsp` with `crates/ridl`.

### 9. Versioning and tagging

`editors/vscode/package.json`'s `version` stays independent of the Rust
workspace's version, as today. A maintainer preparing a release:

1. Bumps `editors/vscode/package.json`'s `version` and commits it.
2. Tags that commit `editor-v<version>` and pushes the tag. The `guard` job
   enforces the match.

The `editor-v*` prefix is a dedicated tag namespace, distinct from the repo-wide
`vX.Y.Z` tags `just release` (`git std bump`) creates, so a workspace release
does not trigger this workflow and this workflow does not tag the workspace.

**This is a second, public release train for the `ridl` binary**, while the
crates it is built from stay at `0.0.0`. Two consequences are handled:

- The binary must identify itself, or a bug report from a Marketplace user names
  nothing. `crates/ridl`'s `build.rs` reads `RIDL_BUILD_VERSION` from the
  environment (set by the release workflow to the tag) and falls back to the
  crate version, so `ridl --version` prints `editor-v0.1.0` from a release build
  and `0.0.0` from a local one. `ridl lsp` reports the same string in its
  `initialize` result.
- The decision to run two trains is recorded in an ADR when this lands — an
  amendment to ADR-0007 decision 14, or a new record — not left implicit in a
  workflow file.

## Maintainer prerequisites

One-time, outward-facing acts a maintainer performs outside this design's code
changes — consistent with ADR-0007 decision 14:

- Add `VSCE_PAT` and `OVSX_PAT` as repository secrets on `driftsys/ridl`. The
  `driftsys` Marketplace publisher and Open VSX namespace already exist
  (established for `markspec-ide`); only tokens scoped to this repository are
  needed.
- Configure a `marketplace` GitHub Environment on `driftsys/ridl` with required
  reviewers, matching the one on `driftsys/markspec`.

## Alternatives considered

- **Two binaries (`ridl-lsp` and `ridl-mcp`)** — the shape of this design's
  first draft. Rejected: markspec, the precedent both specs port, ships one
  binary with `lsp`/`mcp` subcommands; two binaries doubled every bundle and
  install step, shipped nothing for ADR-0005's CLI face, and made the "validate
  the pipeline with two binaries" rationale circular. The cost — the standalone
  `ridl-lsp` binary goes away — is a one-line change to the documented developer
  flow.
- **Marketplace publish via a plain `workflow_dispatch` job.** Rejected in favor
  of the environment-gated approval on the release-triggered workflow: publish
  stays a deliberate human approval without a second workflow the maintainer
  must remember to run.
- **Leaving the binary unbundled** (today's behavior). Rejected: every user
  would build or install `ridl` before the extension does anything, unlike
  `markspec-ide`. `PATH` resolution remains as tier 3 for the unpackaged
  development launch only.
- **VS Code only, no plain binary release.** Rejected once Claude Code and Codex
  were named as hosts: neither reads VS Code's extension-registered MCP
  providers, so a script-installable binary is required to reach them.
- **Deferring the "Install to PATH" command.** Considered, since `install.sh`
  already reaches those hosts. Reversed: one click, no second download, and the
  code exists in markspec.
- **Keeping `engines.vscode` at `^1.91.0`** with hand-declared MCP types and a
  runtime guard. Rejected in section 3; recorded there as the decision to
  revisit if an older fork's users are blocked.

## Open implementation-time risks

- Cross-compiling `aarch64-unknown-linux-gnu` from an `ubuntu-latest` (x86_64)
  runner needs a cross-linker (`cross`, or `gcc-aarch64-linux-gnu` with the
  matching linker environment). The other four targets build natively or with
  `rustup target add` alone.
- The MCP SDK crate for `ridl-mcp` is chosen at implementation time; its async
  runtime lands in the `ridl` binary only, which no gate builds for wasm32.
- Whether `VSCE_PAT` / `OVSX_PAT` are copied from the `markspec-ide` tokens or
  freshly generated is a maintainer decision at setup time.

## Related decisions

- ADR-0007 (E1 execution), decision 14 — release, tagging, and publishing are
  maintainer acts; the second release train above amends it.
- ADR-0009 — gate commands live in the justfile; the workflows invoke them.
- ADR-0005 (agent enablement) — the host and one-binary decisions
  `docs/wip/2026-09-13-ridl-mcp-v0-design.md` records on its behalf.
- `docs/wip/2026-09-13-ridl-mcp-v0-design.md` — the `ridl mcp` subcommand this
  design bundles.
- `docs/ROADMAP.md` — epic E1.17 (VS Code extension, landed), E8.6
  (`ridl_check`), parked block E8.8 (one binary).
