# ADR-0036 — Workspace toolchain pins

## Status

Accepted — 2026-11-25. This record describes the implementation of
[driftsys/ridl#1476](https://github.com/driftsys/ridl/issues/1476). Its scope
comes from the maintainer's amendment of 2026-11-24 and the remaining decisions
delegated on 2026-11-25. It records that design and the resulting behavior; it
does not claim a separate maintainer approval of this ADR.

Scope: root release requirements, their enforcement by workspace consumers,
versioned toolchain management and VS Code selection. Amends
[ADR-0002](ADR-0002-module-system.md) §4,
[ADR-0010](ADR-0010-cli-conventions.md),
[ADR-0025](ADR-0025-workspace-aware-mcp-tools.md) decisions 2, 5 and 7, and
[the MCP workspace tools record](../design/mcp-workspace-tools.md). The as-built
toolchain protocol is in
[the toolchain directory record](../design/toolchain-directory.md).

## Context

Formatting, lock allocation, compatibility classification and generated output
can change between releases. A consumer's installation script could name a
release, but neither workspace loading nor the extension's bundled compiler
checked it. An extension update could therefore change the compiler used for a
workspace without reporting the difference.

The manifest loader already finds the workspace root. Release archives already
exist on the editor release train. These supply the boundary for enforcement and
the source of side-by-side installations without another manifest or proxy.

## Decisions

1. **The root manifest may require a compatible release.** This constrains
   workspace output while allowing compatible patch updates without changing the
   pin. The [release pin chapter](../book/release-pin.md) owns the key's syntax,
   compatibility rule, minimum release and unpinned behavior.
2. **Only the discovered root constrains the consumer.** Entry paths inside
   members use the workspace root found by the existing discovery rule. Member
   pins are ignored, including invalid values, with warning MANI-027, lint name
   `member-ridl-version`. Remote unit pins do not constrain the consumer.
3. **A root refusal excludes other diagnostics and work.** Results from an
   incompatible compiler would defeat the pin, and later validation could add
   unrelated diagnostics. The [release pin chapter](../book/release-pin.md) owns
   refusal diagnostics, loading order and batch exit behavior; its
   [server behavior](../book/release-pin.md#editors-and-agent-tools) owns
   reporting and recovery in LSP and MCP.
4. **Enforcement begins with pin-aware releases.** A new manifest cannot change
   older binaries, so their lack of enforcement is accepted. The
   [release pin chapter](../book/release-pin.md) owns the release boundary and
   older releases' treatment of the key.
5. **Build stamps identify the running release consistently.** The
   [distribution record](../technotes/toolchain-distribution.md#how-the-binary-learns-its-version)
   owns normalization, display and the fallback used by checks. Core loaders
   receive a numeric running release explicitly; they do not read the process
   environment.
6. **Toolchains share immutable version directories.** `ridl toolchain install`,
   `list` and `which` operate independently of workspace compilation. The
   [directory record](../design/toolchain-directory.md) owns the layout,
   completion test, preservation of damage, schema-1 selection and transaction
   ordering. Production CLI downloads use one fixed GitHub editor release origin
   with SHA-256 verification, without a manifest, CLI or environment mirror
   override. Checksums detect altered bytes relative to that record; they do not
   authenticate a publisher independently of the release origin. Installation
   does not modify an existing damaged directory. Releases live under
   `<root>/versions/<version>/`; scripts use the separate
   [`.tmp-script-<version>-<pid>` stage](../design/toolchain-directory.md#install-scripts-and-the-path-copy)
   and do not take the CLI lock.
7. **The extension selects and directly starts one server per RIDL root.** The
   bundled binary resolves toolchains; PATH supplies the resolver only in a
   development launch without a bundle. The extension validates schema and exit
   status before using a result and starts the chosen binary as `ridl lsp`,
   without an intervening proxy. Discovery covers nested roots and members;
   manifestless documents share a separate client. `ridl.serverPath` is an
   explicit folder-specific developer override that bypasses selection and
   installation; its server still enforces pins. The
   [editor behavior](../book/release-pin.md#editors-and-agent-tools) owns
   installation policy, fallback, routing and lifecycle details.
8. **Workspace Trust gates the whole extension.** It declares no support for
   untrusted workspaces, so Restricted Mode starts neither servers nor
   installers. The delegated ruling keeps VS Code's own trust prompt instead of
   activating the extension to show an install prompt: activation would also
   start a server and honor an untrusted workspace's `ridl.serverPath`. Granting
   trust enables normal activation. Automatic installation is an
   application-scoped boolean, default `true`; workspace settings cannot enable
   or redirect downloads.
9. **The window has one MCP definition.** Its binary is selected for the first
   VS Code workspace folder, independently of document-open order, and changes
   when that selection changes. It does not provide one MCP server per RIDL
   root. Terminal-side `ridl mcp` does not hand off to another release.
10. **Follow-ups are bounded and explicit.**
    [driftsys/ridl#1477](https://github.com/driftsys/ridl/issues/1477) tracks
    terminal and MCP hand-off, terminal auto-install, uninstall, signed
    releases, the Workspace Trust install prompt and smaller review items.
    Plugin release pins and automatic pin updates remain outside this change.
    [driftsys/ridl#1478](https://github.com/driftsys/ridl/issues/1478) tracks CI
    running the current extension against the minimum pinnable release; that job
    is due with the first release after **0.23.0**. It is not recorded as
    implemented here.

## Alternatives considered

- **Keep a consumer-specific pin file read only by install scripts.** Rejected:
  it does not constrain workspace commands or the bundled editor compiler.
- **Add `ridl-toolchain.toml`.** Rejected: one value would need a second file
  despite the existing manifest and root discovery.
- **Add a rustup-style proxy manager.** Rejected for this scope: it adds an
  installed component and lifecycle beyond a binary checking its manifest.
- **Have the bundled language server proxy a selected server.** Rejected by the
  scope amendment: the extension can start the selected process directly.
- **Warn on mismatch.** Rejected: output could still differ silently in a job
  that accepts warnings.
- **Allow workspace-controlled download sources or installation policy.**
  Rejected: opening a workspace must not redirect executable downloads.
- **Activate in Restricted Mode just to prompt for installation.** Deferred by
  decision 8; the current extension remains inactive until trust is granted.

## Consequences

- A pin gives a checked release boundary to workspace consumers, while several
  releases can remain installed on one machine. The terminal user invokes the
  selected path explicitly until hand-off is implemented.
- Damaged installations need an explicit user action to preserve or move them;
  an installer does not repair them automatically.
- New manifest keys, diagnostic codes and CLI/schema entries extend frozen
  surfaces by addition. This decision removes no baseline contract.
- A newer extension must retain compatibility with the first pinnable release;
  decision 10 records when the corresponding CI coverage is due.

## References

- [Release pin chapter](../book/release-pin.md) — user behavior and coverage.
- [Toolchain directory](../design/toolchain-directory.md) — the implemented
  protocol and its regression tests.
- [Distribution record](../technotes/toolchain-distribution.md) — release
  trains, build stamps, scripts and editor module ownership.
- [Architecture map](../technotes/walking-skeleton-architecture.md) — crate
  ownership.
- [Family overview](../specification/ridl-family-overview.md) — decision ledger
  and manifest diagnostic catalogue.
