# Pin the ridl release per workspace — design

Issue: driftsys/ridl#1476 (the issue body is part 1 and the original part 2; the
maintainer's amendment comment of 2026-11-24 replaces part 2 and sets the scope
of 0.23.0). The maintainer delegated the remaining decisions on 2026-11-25;
every call that is not in the issue or the amendment is marked **(call)** with
its reason.

## Intent

A workspace cannot say which `ridl` release it is built with, and every release
accepts every workspace. `ridl fmt` layout, `ridl lock` allocation, `ridl diff`
classification and generated code change between releases, so a developer whose
`ridl` differs from CI's gets different output with no diagnostic. In one
consumer repository the pin (kept in a file of its own) was 0.17.0 and a laptop
ran 0.22.0, and nothing reported it. The editor is worse: the extension runs its
bundled binary, so an extension update changes the compiler in the editor.

Success: a workspace states its release in `ridl.toml`; every command, `ridlc`
and the language server refuse an incompatible release; the editor runs the
pinned release without a manual step; several releases live side by side on one
machine.

## Goals

1. An optional `ridl-version` key in `ridl.toml`, checked by every command that
   loads a manifest, by `ridlc` and by `ridl lsp`.
2. `ridl toolchain install`, `list` and `which`, over one directory per release.
3. `install.sh` and `install.ps1` install into that same directory.
4. The VS Code extension installs the pinned release when it is missing and
   starts it as the language server, with a fallback to the bundled release.

## Non-goals (moved to a later release, own issue)

- `ridl` in a terminal running the pinned release in place of itself (hand-off,
  exit code forwarding, Windows Job Object, a guard against a second hand-off).
- The same for `ridl mcp`.
- An opt-in automatic install in the terminal.
- `ridl toolchain uninstall`.
- Verification of signed release artifacts or GitHub artifact attestations.
- Pinning plugin releases (`ridlc-gen-kotlin`).
- Updating the pin automatically.

## Anti-goals

- A mismatch that only warns. The pin is useful only when a mismatch fails.
- A download that a workspace can cause or redirect: no URL, mirror or
  auto-install switch is read from `ridl.toml` or from a workspace setting.
- A download in a folder the user has not trusted.
- A process between the editor and the language server.
- A partially installed release that counts as installed.

## Constraints

- ADR-0010: exit 0 success, 1 a negative answer, 2 the tool could not answer.
  Each new subcommand adds a row to its table.
- ADR-0029: the manifest keys, the diagnostic codes and the CLI flags are frozen
  surfaces with dumps under `freeze/`. This change only adds lines.
- The release assets are `ridl-<target>.tar.gz` and `<name>.tar.gz.sha256` under
  the tag `editor-v<version>`, flat archives with the binary at the root, for
  `x86_64`/`aarch64-unknown-linux-gnu`, `x86_64`/`aarch64-apple-darwin` and
  `x86_64-pc-windows-msvc`.
- `ureq`, `tar` and `sha2` are already dependencies (the remote import fetch in
  `ridl-core`, feature `fetch`). `flate2` is only a transitive dependency; the
  release archives are gzip, so the toolchain code adds `flate2` as a direct
  dependency of `crates/ridl` **(call)**, with its notice in the third-party
  notices file if the notice generation does not pick it up by itself.
- The toolchain code (download, lock, extraction) lives in `crates/ridl`, not in
  `ridl-core`, `ridl-ir` or `ridl-descriptor`, so `just compat-check`'s older
  `rust-version` never builds it. The pin parsing and the compatibility rule
  live in `ridl-core` and use no API newer than that crate already uses.
- The minimum pinnable release is 0.23.0.
- A release older than the pin's release still reads `ridl-version` as an
  unknown key (MANI-005, a warning) and runs; releases before 0.23.0 cannot be
  changed. From 0.23.0 on, an older `PATCH` or a different `MAJOR.MINOR` is
  refused. This is accepted, and the ADR states it.
- No manifest in this repository's examples, book or fixtures carries
  `ridl-version`, except test fixtures that inject the running release: a
  development build of `main` reports the previous release until the release
  commit bumps it.

## Approaches

The maintainer chose among these in the issue and the amendment.

1. **Status quo: a pin file in each consumer repository**, read by its install
   scripts. Works only where a script reads it; the editor never does.
2. **A separate `ridl-toolchain.toml` read by the CLI.** A second file for one
   value; the code that loads `ridl.toml` already exists.
3. **A rustup-style version manager with a proxy binary.** More to install and
   maintain than one binary that checks its own manifest.
4. **Chosen: a key in `ridl.toml`, checked by the loader; a versions directory
   managed by `ridl toolchain`; the extension selects and starts the pinned
   binary itself.** Hand-off in the terminal comes later.

For the editor, the amendment rejected an LSP proxy (the bundled `ridl lsp`
starting the pinned one) in favour of the extension starting the pinned binary,
so no process sits between the editor and the server.

## Design

### 1. The `ridl-version` key

- Allowed in the `[workspace]` table and in the `[package]` table. The value is
  a string `MAJOR.MINOR.PATCH`, three decimal numbers, no prefix, no pre-release
  or build suffix.
- Only the **root manifest** (the one `find_root` returns) pins. **(call)** A
  member package manifest that holds the key gets a warning, MANI-027, and the
  key is ignored: two pins in one workspace would need a rule for which wins,
  and the release is a property of the workspace.
- Compatibility: the running release is compatible when its `MAJOR.MINOR` equals
  the pin's and its `PATCH` is greater than or equal to the pinned one.
- New diagnostic codes **(call — the next free numbers)**:

  | Code     | Level   | Meaning                                                                    |
  | -------- | ------- | -------------------------------------------------------------------------- |
  | MANI-024 | Error   | `ridl-version` is not a string of the form `MAJOR.MINOR.PATCH`.            |
  | MANI-025 | Error   | `ridl-version` is below 0.23.0, the minimum pinnable release.              |
  | MANI-026 | Error   | The running release is not compatible with `ridl-version`.                 |
  | MANI-027 | Warning | `ridl-version` in a member package manifest; only the workspace root pins. |

  The MANI-026 message names the pinned release, the running release and the
  command `ridl toolchain install <pin>`. All four sit on the key's value span.
- Order: the pin is checked **first**, from the root manifest's TOML, before any
  other manifest validation. **(call)** When MANI-024, MANI-025 or MANI-026 is
  reported, nothing else of the workspace is reported or analyzed: a newer
  release's keys would otherwise show as MANI-005 noise, and results from a
  different compiler than CI's are what the pin exists to prevent. Invalid TOML
  is still MANI-001.
- Where **(call)**: `find_root` is unchanged. The workspace loader entry points
  in `ridl-core` (the functions every caller already uses after `find_root`,
  `load_workspace*`) run the pin check on the root manifest right after the root
  is known and before members are read, and return only the pin diagnostic when
  it refuses. If the root manifest is not valid TOML, the pin check is skipped
  and the loader reports MANI-001 as today. A caller that loads a manifest
  through another path (the language server's own load, `ridlc`) is routed
  through the same function, not given a copy.
- Exit code: any of MANI-024, -025, -026 makes a command exit 2 (the tool cannot
  answer for this workspace), whatever its usual exit for a diagnostic error.
  **(call)** `ridl-core` exposes one predicate (for example
  `is_release_refusal(&Diagnostic) -> bool`) and each command's exit mapping in
  `main.rs` and in `ridlc` checks it first. `ridl-core`'s Rust API is not a
  frozen surface; the exit codes are, and this only adds a case that did not
  exist.
- Who checks: every command and tool that loads a workspace manifest — `check`,
  `build`, `fmt`, `lock`, `diff`, `baseline`, `test`, `vendor`, `describe`,
  `ridlc`, `ridl lsp` and `ridl mcp`. `ridl init`, `ridl new` and
  `ridl toolchain` do not check (`toolchain which` reads the pin and reports it
  instead).
- A manifest without the key behaves as today.
- The running release, for the check, is the binary's own `MAJOR.MINOR.PATCH`.
  **(call)** `crates/ridl/build.rs` strips a leading `editor-v` or `v` from
  `RIDL_BUILD_VERSION`, so `ridl --version` prints `ridl 0.23.0` instead of
  `ridl editor-v0.23.0`, and the LSP and MCP `serverInfo.version` carry the same
  plain value. `ridlc` gets the same stamp (a `build.rs` of its own, reading the
  same variable the release workflow already sets for both binaries), so
  `ridlc --version` and its check use the same plain version. The check needs a
  plain version, and the prefix is a tag name, not a version. Docs or tests that
  quote the prefixed form are updated.
- Testability **(call)**: the check takes the running release as a parameter.
  Integration tests may override the running release through a mechanism that
  does not exist in a release build (for example an environment variable read
  only under `cfg(debug_assertions)`), because a development build of `main`
  reports the previous release's version until the release commit bumps it.

### 2. Language server

- On MANI-024, -025 or -026, `ridl lsp` publishes the diagnostic on the key in
  `ridl.toml` and does not analyze the workspace (no diagnostics, hovers or
  completions from its sources). It keeps running, so the diagnostic stays
  visible. The diagnostic is published for the `ridl.toml` URI through the path
  the server already uses for manifest diagnostics; where a load failure today
  only shows a message, the refusal is published as a diagnostic instead.
- The server loads again when the root `ridl.toml` changes on disk or in an open
  buffer. If the server does not already reload on that event, the extension
  registers a file watcher for `**/ridl.toml` and the server handles the change,
  so fixing the pin clears the refusal without a restart.
- **(call)** When the root manifest has no `ridl-version`, the server offers a
  code action on `ridl.toml`, "Pin this workspace to ridl <running release>",
  that inserts `ridl-version = "<running>"` into the `[workspace]` or
  `[package]` table. It lives in the server, not the extension, so every editor
  gets it. It draws no diagnostic, so an unpinned workspace shows no noise.

### 3. The versions directory

- Root **(call)**:
  - `RIDL_TOOLCHAIN_DIR` when set (CI caches and tests; an environment variable
    is not something a workspace can write);
  - Linux: `$XDG_DATA_HOME/ridl`, else `~/.local/share/ridl`;
  - macOS: `~/Library/Application Support/ridl`;
  - Windows: `%LOCALAPPDATA%\ridl`.
- A release lives in `<root>/versions/<MAJOR.MINOR.PATCH>/`, holding the files
  of the release archive (`ridl` or `ridl.exe`, `LICENSE`,
  `THIRD-PARTY-NOTICES.txt`) and an empty completion marker `.complete`.
- A release counts as installed only when its directory and `.complete` both
  exist.
- An existing version directory is never written to. A directory without
  `.complete` (left by a manual copy) is reported as damaged, with its path, and
  `install` refuses to touch it **(call — no uninstall in this release, and
  deleting a directory the tool did not create is not safe)**.

### 4. `ridl toolchain`

`ridl toolchain install [<version>]`

- `<version>` is an exact `MAJOR.MINOR.PATCH` at or above 0.23.0. Without it,
  the pin of the workspace at or above the current directory; no pin is an
  error, exit 2.
- Already installed: prints the path, exit 0, no network.
- Otherwise: take a lock on `<root>/versions/.<version>.lock` (std
  `File::lock`); after acquiring it, check again whether the release is
  installed (a second process waits, then finds it installed). Download
  `ridl-<target>.tar.gz` and its `.sha256` from
  `https://github.com/driftsys/ridl/releases/download/editor-v<version>/`,
  compare the SHA-256, extract into `<root>/versions/.tmp-<version>-<pid>/`
  (same file system), write `.complete`, rename the directory to `<version>`.
  Any failure removes the temporary directory and exits 2.
- `<target>` is the target the running binary was built for. A target with no
  release archive (a musl build, an unsupported architecture) fails with a
  diagnostic that names the target, exit 2.
- The origin is a constant in the binary. **(call)** Tests reach the download
  code through a library function that takes the base URL as a parameter; a
  release binary has no way to change the origin.
- Progress and errors go to stderr; the installed path goes to stdout.

`ridl toolchain list`

- One line per installed release, ascending version order, `<version>\t<path>`
  to stdout. A damaged directory is listed with `(damaged)`. Exit 0, also when
  empty.

`ridl toolchain which [<path>] [--json]`

- `<path>` defaults to `.`, like the other subcommands, and locates the root
  manifest with `find_root`.
- It selects the **highest** compatible release among the installed ones and the
  running binary itself **(call — the running binary is a valid answer, which
  spares a download when the bundled release already matches)**.
- `--json` writes one object to stdout:

  ```json
  {
    "schema": 1,
    "manifest": "/abs/ridl.toml",
    "pin": "0.23.0",
    "running": "0.23.2",
    "status": "compatible",
    "path": "/abs/versions/0.23.4/ridl",
    "source": "installed",
    "diagnostic": null
  }
  ```

  `status` is one of `compatible`, `missing`, `unpinned`, `invalid`. `manifest`
  and `pin` are `null` when absent; `path` and `source` (`installed` or
  `running`) are set only for `compatible`; `diagnostic` holds `{code, message}`
  for `invalid` (MANI-024 or -025). With `unpinned`, `path` is the running
  binary and `source` is `running`, so a caller always has a binary to start.
- Without `--json`, the path alone for `compatible` and `unpinned`, and a
  sentence on stderr for the others.
- Exit: 0 for `compatible` and `unpinned`; 1 for `missing` and `invalid` (a
  negative answer); 2 when the manifest cannot be read.

The three subcommands each get a row in ADR-0010's table, an entry in
`docs/book/cli-reference.md`, and lines in `freeze/cli.txt`.

### 5. `install.sh` and `install.ps1`

- They install into `<root>/versions/<version>/` with the same root rules,
  `.complete` marker and temporary-directory rename as `ridl toolchain install`.
  An already complete version directory is reused without download.
- They then copy **(call — a copy keeps today's behaviour for users who delete
  or move files; a symlink would break silently)** the binary to the `PATH`
  directory as today (`RIDL_INSTALL_DIR`, default `~/.local/bin` or
  `%LOCALAPPDATA%\Programs\ridl`), so the `ridl` on `PATH` stays the default
  release.
- `RIDL_VERSION` accepts `0.23.0` as well as the tag form `editor-v0.23.0`.
- **(call)** The versions directory holds `ridl` only, because only `ridl` is
  selected by `toolchain which`. `RIDL_INSTALL_BINARY=ridlc` keeps today's flat
  install into the `PATH` directory and does not touch the versions directory.
- `just install-check` covers the new layout: the version directory and marker
  exist, the `PATH` copy is byte-identical, a second run does not download, and
  a tampered tarball leaves no version directory.

### 6. The VS Code extension

Binary selection, per workspace folder:

1. `ridl.serverPath` set: use it as today, with no toolchain logic. Its server
   reports MANI-026 itself when the pin does not match.
2. Otherwise run `<bundled> toolchain which --json <folder>` (bundled, else
   `ridl` on `PATH` when the VSIX has no bundled binary).
3. `compatible` or `unpinned`: start `path` with `lsp`.
4. `invalid`: start the bundled binary; its server shows MANI-024 or -025.
5. `missing`: when `ridl.toolchain.autoInstall` is on, run
   `<bundled> toolchain install <pin>` under a progress notification (shown for
   the first install of each release, since a second install finds it
   installed), run `which` again, and start the result. When the setting is off,
   start the bundled binary and show a notification with an **Install** button
   that runs the same install.
6. Install failure or no network: start the bundled binary, whose server shows
   MANI-026 on the key, and show the install error once.

Rules:

- **Workspace Trust (call).** The extension keeps declaring no
  `untrustedWorkspaces` capability, so VS Code does not activate it in an
  untrusted folder: nothing runs, nothing downloads, and VS Code's own
  Restricted Mode banner is the prompt. The amendment asked for an install
  prompt in an untrusted folder; enabling the extension in Restricted Mode to
  show it would also start a language server and honour `ridl.serverPath` from
  an untrusted `.vscode/settings.json`, which is a larger exposure than the
  prompt is worth. When the user grants trust, the extension activates and the
  normal flow runs.
- **`ridl.toolchain.autoInstall`**: boolean, default `true`,
  `"scope": "application"`, so only user settings can set it.
- **Several workspaces in one window (call)**: the extension does not walk
  directories itself. For each opened RIDL document whose directory is not yet
  covered, it runs `which --json <document directory>`; the returned `manifest`
  names the RIDL workspace root, which is the same rule `find_root` applies in
  the server. One language client runs per root, with a document selector scoped
  to that root's directory and the binary that root selects. Documents with no
  manifest share one client on the bundled binary. This covers multi-root VS
  Code workspaces (the amendment's "one server per folder") and also two RIDL
  workspaces inside one folder.
- **Pin change**: a change to a root `ridl.toml` runs the selection again for
  that root and restarts its client when the selected binary changed.
- **MCP**: the MCP server definition the extension registers uses the binary
  selected for the first workspace folder **(call — one MCP server per window;
  terminal-side hand-off for `ridl mcp` is a later release)**.
- `which` is parsed against `schema: 1`; an unknown schema falls back to the
  bundled binary with a logged warning.

### 7. Records

- A new ADR (next number) records the pin, the compatibility rule, the minimum
  release, the versions directory, the editor's selection and the trust rule,
  and the deferred items with their issue number.
- The manifest reference (`docs/book/`) documents the key, the rule and the four
  codes; `docs/book/getting-started.md` documents the versions directory and the
  install scripts; the extension README documents the setting.
- A design record under `docs/design/` describes the toolchain directory,
  `which --json` and the install protocol.
- **(call)** "CI runs the current extension against the minimum release" has no
  effect while the minimum is the current release. An issue is filed for the CI
  job, to land with the first release after 0.23.0.
- An issue is filed for the deferred items listed under Non-goals.

## Left to the builder

- The module layout (where the toolchain code lives in `crates/ridl`, how the
  pin check is threaded through the loader to each caller).
- How `ridlc` gets its running release, as long as it is the same
  `MAJOR.MINOR.PATCH` as `ridl`'s.
- The exact message wording of the four codes.
- The extension's internal structure for per-folder clients.
- The test seam for the download (a local HTTP server or `file://`).
