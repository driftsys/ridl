# Toolchain distribution as built

How the `ridl` binary and the VS Code extension reach a user's machine, and how
the extension finds the binary once it is there. Informative: the normative
decisions are ADR-0007 decision 14 and its 2026-09-13 amendment (the release
train and the maintainer acts) and ADR-0005's resolved host-coverage question
(which agent hosts are first-class, and the VS Code version floor that follows).

## Two release trains, disjoint tag namespaces

The repository has two independent release trains:

- **`v<version>`**, created by `just release` after `git std bump` bumps the
  `[workspace.package]` version every crate in the workspace shares (ADR-0007
  decision 14's 2026-09-21 amendment), and pushed by `just release-push`. The
  tag triggers `.github/workflows/crates-io-release.yml`, which publishes every
  crate whose manifest does not set `publish = false` to crates.io: a `verify`
  job checks the tag's commit is an ancestor of `main` and runs `just build`,
  then a `publish` job runs the crates in dependency order, skipping one already
  at that version on crates.io. `xtask`, `ridlc-gen-model` and `ridlc-gen-rust`
  are the crates left `publish = false`: the first is workspace tooling and the
  other two are test-only, so none is published; every other crate, including
  `ridl` (published under the crates.io package name `ridl-cli` — `ridl` is
  already someone else's crate — with an explicit `[[bin]] name = "ridl"` so the
  installed binary is unchanged), `ridl-lsp` and `ridl-mcp`, is on this train
  (ADR-0007 decision 14's 2026-09-22 amendment). `ridl-rt` used to have a
  version and a tag of its own (`ridl-rt@<version>`,
  [R-12 of the archived spec](../archive/2026-09-13-ridl-rt-v0.1-design.md)); it
  now shares the workspace version and this tag like every other published
  crate. The tag push is still the maintainer act (ADR-0007 decision 14).
- **`editor-v<version>`**, created on the same release commit by `just release`
  and pushed by `just release-push` (see "Cutting a release"). This builds the
  `ridl` binary for five targets and packages one VSIX per target, creates the
  GitHub Release, and holds the Marketplace publish behind the reviewed
  `marketplace` environment (`.github/workflows/vscode-release.yaml`).

The two namespaces are disjoint on purpose, and one consequence is easy to get
wrong: `install.sh` and `install.ps1` must **list** releases and take the newest
whose tag starts with `editor-v`. They must never ask for `/releases/latest`,
which returns the newest release across every tag namespace — once the workspace
train is used, that would hand a user a release containing no `ridl` binary.

The two Linux targets are built with `cargo zigbuild` against a glibc 2.28 floor
(`--target <triple>.2.28`), so the binary starts on any distribution with glibc
2.28 or newer, which includes RHEL 8, Debian 11 and Ubuntu 20.04. A binary
linked against the runner's own glibc would require that newer version instead,
and the extension cannot recover from that: it does not fall back to `PATH`
while a bundled binary exists. The floor changes neither the target triple nor
the tarball names, so `install.sh` and the VSIX layout are unaffected.

## Cutting a release

The two trains are cut together. The editor version is the workspace version,
because `vscode-release.yaml` refuses an `editor-v` tag that differs from
`editors/vscode/package.json`. The release commit goes to `main` directly,
without a pull request.

1. On a clean `main` equal to `origin/main`, whose `ci` run has passed, run
   `just release --release-as <version>`. Before 1.0, `git std bump` picks a
   patch release unless it is told the version. The recipe runs the release
   check of the freeze gate, then `git std bump --no-commit`, then
   `just release-sync`, which updates the pins that a `[[version_files]]` entry
   cannot express: the root and cabin lockfiles, the `ridl-rt` requirement that
   `ridl build` emits, the cabin manifests and `deno.lock`, the editor manifest
   and the frozen generated manifests. The recipe does not run the build gate,
   which `ci` already ran on that commit. It checks only what the bump and the
   sync wrote: that both lockfiles agree with their manifests
   (`cargo metadata --locked`) and that every derived pin follows the new
   version. It then commits `chore(release): <version>`, and creates the
   annotated tags `v<version>` and `editor-v<version>`, all locally.
2. Read the changelog in the release commit.
3. Run `just release-push`. It pushes `main`, waits for the `ci` run on the
   release commit, and pushes the two tags only if that run passed. A push to
   `main` during that run cancels it; re-run it with `gh run rerun` and run
   `just release-push` again, which does not push the release commit a second
   time. If the run failed, fix `main` and cut the next patch version. Keep the
   two local tags and never push them: `git std bump` starts the next changelog
   section after the newest tag, so without them the next section repeats every
   entry of the failed version. The tags start `crates-io-release.yml`,
   `npm-release.yml` and `vscode-release.yaml`. The Marketplace job then waits
   for approval in the `marketplace` environment.

A tag is not moved after it is pushed. A publish workflow that starts from a tag
runs that tag's commit, and a re-run of it also uses the workflow file of that
commit, so a fix on `main` cannot reach it. A failure in a publish workflow
before anything is published is fixed on `main` and released as the next patch
version. The crates.io and npm jobs skip a version that is already on their
registry, so a re-run after a partial publish continues where the earlier run
stopped. The Marketplace publish is not idempotent: after a partial failure, the
remaining targets are published by hand, as `vscode-release.yaml` describes. The
crates.io job waits after each crate until the version is in the index that
cargo reads (`.github/scripts/wait-for-crate-index.sh`).

## How the binary learns its version

The `ridl` crate shares the workspace version like every other crate, but that
version is not what a user sees: it moves on the crates.io train's `v*` tags,
which say nothing about the CLI binary. Both `crates/ridl/build.rs` and
`crates/ridlc/build.rs` read `RIDL_BUILD_VERSION` at build time, trim
whitespace, and remove one leading `editor-v` or `v`. Absent or blank input uses
the crate version. The release workflow supplies the `editor-v*` tag, so both
binaries report its unprefixed version. `ridl --version`, LSP `serverInfo` and
MCP server info share that display string. Pin checks use the crate version if
the display string is not a plain release. See
[workspace release pins](../book/release-pin.md).

## Versioned toolchains

`ridl toolchain install`, `list` and `which` manage and inspect a shared
versions directory independently of workspace compilation. The
[release pin chapter](../book/release-pin.md#installing-and-selecting-toolchains)
owns root precedence and command usage. The
[toolchain directory record](../design/toolchain-directory.md) owns layout,
selection schema and installation protocol. The installer uses the build target
exported by Cargo, and the fixed GitHub `editor-v<version>` release origin. Its
alternate download source is a library test parameter, with no CLI, manifest or
environment override.

`just install-check` also runs the ignored
`toolchain_ureq_path_verifies_and_installs` local HTTP test, exercising the
actual ureq path and checksum refusal. File-based fixture tests cover
extraction, cleanup, damaged directories and concurrent installers without a
network listener. The install scripts below produce the shared layout for
pinnable `ridl` releases.

## Editor toolchain selection

`editors/vscode/src/binaryResolution.ts` resolves the bootstrap binary from
`ridl.serverPath`, the bundled `bin/ridl` (`bin/ridl.exe` on Windows), or PATH.
The pure `toolchainSelection.ts` validates the CLI selection report and manages
installation through that bootstrap binary. `workspaceClients.ts` associates
document directories with discovered roots and owns their client lifecycles.
`extension.ts` connects these modules to VS Code document events, manifest
watching, notifications and the MCP registry.

The [release pin chapter](../book/release-pin.md#editors-and-agent-tools) owns
the editor's selection, installation policy, routing, override and trust
behavior, including the first-folder MCP limitation. Language clients run the
selected binary as `ridl lsp --stdio`; the MCP definition runs its selection as
`ridl mcp` (ADR-0010).

## "Install ridl to PATH"

The extension command copies its bundled binary to a directory on the user's
`PATH`. Two details in `editors/vscode/src/installToPath.ts` are there for a
reason:

- **The copy goes through a temporary file in the target directory and is then
  renamed over the target.** A direct copy onto the destination truncates it
  before writing, which fails while a previous copy of the binary is still
  running, and can leave a truncated file on `PATH` if it is interrupted. The
  rename is atomic on POSIX.
- **Staleness is a version comparison, not a timestamp.** `copyIsStale` compares
  the `ridl --version` output of the bundled binary against the installed one
  and reports stale only when both versions are known and the installed one is
  strictly older. A newer installed copy is never offered for a downgrade. An
  unknown version on either side is not treated as stale, so a binary the probe
  could not read is never replaced silently.

## The install scripts

`install.sh` and `install.ps1` install the binary without the extension, which
is what Claude Code and Codex need for their MCP configuration. They discover
the newest editor release or use `RIDL_VERSION`, accepting `0.23.0` and
`editor-v0.23.0`. Explicit historical tags remain supported. Dry runs print the
resolved URL without creating directories or downloading.

For `ridl` 0.23.0 and later, the scripts download the target archive and SHA-256
record, verify the digest and archive contents, and install into the shared
[toolchain directory](../design/toolchain-directory.md). That record owns the
layout and completion rules; the
[book](../book/release-pin.md#installing-and-selecting-toolchains) owns root
configuration. Complete releases are reused offline; damaged directories are
preserved and named in the error. The scripts then copy the binary to
`RIDL_INSTALL_DIR`, defaulting to `$HOME/.local/bin` or
`%LOCALAPPDATA%\Programs\ridl`. PATH entries are regular files. Older releases,
historical tags without a plain release version, and `RIDL_INSTALL_BINARY=ridlc`
retain flat installation in that PATH directory.

`just install-check` first runs the ignored toolchain HTTP test, then checks the
scripts with fixture releases at `editor-v0.23.0`: bare/tag normalization,
shared layout, offline reuse, damaged-directory preservation, checksum and
archive refusal, platform roots and paths containing spaces. It also checks flat
`ridlc` and historical `ridl` installation, invalid binary selection, dry runs
and Deno ZIP verification. Bash fixtures use `file://`; the optional PowerShell
branch uses a local HTTP server. The full recipe requires permission to bind
local TCP listeners, including when `pwsh` is absent.
