# VS Code Extension and Toolchain Distribution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship the `ridl` binary and the VS Code extension: a tag-triggered
GitHub Release with five per-platform `.vsix` files and binary tarballs,
environment-gated Marketplace and Open VSX publishing,
`install.sh`/`install.ps1`, and an "Install to PATH" command — with the
extension bundling `ridl` and spawning it as `ridl lsp` and `ridl mcp`.

**Architecture:** The extension gains one pure resolution module (setting →
bundled `bin/ridl` → `PATH`) used by both the LSP client and a new MCP
server-definition provider, plus a ported install-to-PATH module. Two new GitHub
workflows invoke two new `just` recipes; the release workflow builds `ridl` per
target with the tag injected as its version, packages a `.vsix` per target,
creates the release, and holds the publish jobs behind a `marketplace`
environment.

**Tech Stack:** TypeScript 5.9 (`Node16` modules, `node:test` runner),
`vscode-languageclient` 10, `@vscode/vsce` 3.9, `ovsx`, VS Code API 1.101
(`lm.registerMcpServerDefinitionProvider`), GitHub Actions
(`softprops/action-gh-release`, `taiki-e/setup-cross-toolchain-action`), Rust
`build.rs`, bash and PowerShell.

**Spec:** `docs/wip/2026-09-13-vscode-extension-distribution-design.md` — read
it first. **Prerequisite:** `docs/wip/2026-09-13-ridl-mcp-v0-plan.md` is
complete on this branch (`ridl lsp` and `ridl mcp` exist).

## Global Constraints

- Same branch and worktree as the ridl-mcp v0 plan; never `main`; one PR at the
  end (AGENTS.md).
- Conventional Commits; scopes used here: `ridl`, `ridl-lsp`, `editors`, `ci`,
  `repo`, `docs`, `adr`.
- Gate commands live in the justfile; workflows invoke recipes (ADR-0009). The
  two new recipes are **not** members of `just build`, and the justfile says so.
- `just build` stays green after every task; `cargo fmt --all` before every Rust
  commit; `just fmt` before every Markdown/JSON/YAML commit.
- Manifest floor: `engines.vscode` `^1.101.0`, `@types/vscode` `1.101.0` (spec
  §3 — accepted with the fork consequence recorded there).
- Tag namespace: `editor-v<version>`, where `<version>` equals
  `editors/vscode/package.json` `version` (spec §9).
- Binary name in every artifact: `ridl` (`ridl.exe` on Windows). Tarball:
  `ridl-<target>.tar.gz` + `.sha256`. VSIX: `ridl-vscode-<vsce-target>.vsix`.
- Five targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`, `x86_64-pc-windows-msvc`; vsce
  targets in the same order: `linux-x64`, `linux-arm64`, `darwin-x64`,
  `darwin-arm64`, `win32-x64`.
- Nothing in this plan pushes a tag, publishes to a registry, or adds a secret:
  those are maintainer acts (ADR-0007 d14). The PR description carries the
  maintainer checklist (Task 9).
- Prose in code, commits, and docs is plain and literal (AGENTS.md).

---

### Task 1: Inject the release version into the `ridl` binary and the LSP `serverInfo`

**Files:**

- Create: `crates/ridl/build.rs`
- Modify: `crates/ridl/src/main.rs:44` (the `#[command(...)]` attribute)
- Modify: `crates/ridl-lsp/src/server.rs:51-61` (`run`)
- Modify: `crates/ridl/src/main.rs` (`run_lsp`, from the ridl-mcp plan)
- Test: `crates/ridl/tests/version.rs` (new), `crates/ridl-lsp/tests/server.rs`
  (one new test)

**Interfaces:**

- Consumes: `lsp_server::Connection::{initialize_start, initialize_finish}`;
  `server_capabilities()` in `server.rs`.
- Produces: the compile-time env `RIDL_BUILD_VERSION` (the `editor-v*` tag, or
  the crate version `0.0.0` when unset); `ridl --version` prints `ridl <that>`;
  `pub fn run_with_version(connection: Connection, version: Option<&str>) -> Result<(), Error>`
  in `ridl_lsp::server`, with `run` delegating to it with `None`.

- [ ] **Step 1: Write the failing tests**

Create `crates/ridl/tests/version.rs`:

```rust
//! `ridl --version` reports the build version: the `editor-v*` tag when the
//! release workflow set `RIDL_BUILD_VERSION`, else the crate version.

use std::process::Command;

#[test]
fn version_reports_the_build_version() {
    let expected = std::env::var("RIDL_BUILD_VERSION")
        .unwrap_or_else(|_| env!("CARGO_PKG_VERSION").to_string());
    let output = Command::new(env!("CARGO_BIN_EXE_ridl"))
        .arg("--version")
        .output()
        .expect("run ridl --version");
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        format!("ridl {expected}")
    );
}
```

Add to `crates/ridl-lsp/tests/server.rs` (next to the other tests; it uses the
same `Connection::memory()` pattern the file already uses at line 271):

```rust
#[test]
fn initialize_reports_the_server_version_when_given_one() {
    use lsp_server::{Connection, Message, Notification, Request, RequestId};
    use serde_json::json;

    let (server_side, client) = Connection::memory();
    let server = std::thread::spawn(move || {
        ridl_lsp::server::run_with_version(server_side, Some("editor-v1.2.3"))
    });

    client
        .sender
        .send(Request::new(RequestId::from(1), "initialize".to_string(), json!({ "capabilities": {} })).into())
        .unwrap();
    let response = match client.receiver.recv().unwrap() {
        Message::Response(response) => response,
        other => panic!("expected the initialize response, got {other:?}"),
    };
    let result = response.result.expect("initialize succeeds");
    assert_eq!(result["serverInfo"]["name"], "ridl-lsp");
    assert_eq!(result["serverInfo"]["version"], "editor-v1.2.3");
    assert!(result["capabilities"].is_object());

    client
        .sender
        .send(Notification::new("initialized".to_string(), json!({})).into())
        .unwrap();
    client
        .sender
        .send(Request::new(RequestId::from(2), "shutdown".to_string(), json!(null)).into())
        .unwrap();
    // Drain until the shutdown response, then exit.
    loop {
        if let Message::Response(response) = client.receiver.recv().unwrap()
            && response.id == RequestId::from(2)
        {
            break;
        }
    }
    client
        .sender
        .send(Notification::new("exit".to_string(), json!(null)).into())
        .unwrap();
    server.join().unwrap().unwrap();
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:
`cargo test -p ridl --test version && cargo test -p ridl-lsp initialize_reports`
Expected: the first fails (`ridl --version` prints `ridl 0.0.0` and
`RIDL_BUILD_VERSION` is unset — it passes vacuously; confirm the real failure
with `RIDL_BUILD_VERSION=editor-v9.9.9 cargo test -p ridl --test version`, which
fails because the binary still prints `0.0.0`); the second fails to compile
(`run_with_version` missing).

- [ ] **Step 3: Add the build script and the version plumbing**

Create `crates/ridl/build.rs`:

```rust
//! Stamps the binary with its release version. The release workflow sets
//! `RIDL_BUILD_VERSION` to the `editor-v*` tag; a local build reports the
//! crate version, so a bug report can name the build it came from.

fn main() {
    println!("cargo:rerun-if-env-changed=RIDL_BUILD_VERSION");
    let version = std::env::var("RIDL_BUILD_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| env!("CARGO_PKG_VERSION").to_string());
    println!("cargo:rustc-env=RIDL_BUILD_VERSION={version}");
}
```

In `crates/ridl/src/main.rs` change the command attribute to:

```rust
#[command(name = "ridl", about = "The RIDL toolchain", version = env!("RIDL_BUILD_VERSION"))]
```

In `crates/ridl-lsp/src/server.rs` replace `run` with:

```rust
/// Runs the server over `connection` until the client shuts it down: the
/// initialize handshake (including the `initialized` notification), one
/// workspace load, the initial diagnostics publish, then the message loop.
pub fn run(connection: Connection) -> Result<(), Error> {
    run_with_version(connection, None)
}

/// [`run`], reporting `version` in the initialize result's `serverInfo` when
/// one is given (the `ridl` binary passes its build version).
pub fn run_with_version(connection: Connection, version: Option<&str>) -> Result<(), Error> {
    let (id, params) = connection.initialize_start()?;
    let params: lt::InitializeParams = serde_json::from_value(params)?;
    let mut result = serde_json::json!({
        "capabilities": server_capabilities(),
        "serverInfo": { "name": "ridl-lsp" },
    });
    if let Some(version) = version {
        result["serverInfo"]["version"] = serde_json::Value::String(version.to_string());
    }
    connection.initialize_finish(id, result)?;
    let mut state = ServerState::new(workspace_root(&params));
    state.publish_all(&connection)?;
    main_loop(connection, state)
}
```

In `crates/ridl/src/main.rs`, in `run_lsp`, change
`ridl_lsp::server::run(connection)` to
`ridl_lsp::server::run_with_version(connection, Some(env!("RIDL_BUILD_VERSION")))`.

- [ ] **Step 4: Run the tests to verify they pass**

Run:
`cargo test -p ridl-lsp && cargo test -p ridl --test version && RIDL_BUILD_VERSION=editor-v9.9.9 cargo test -p ridl --test version`
Expected: all pass, including every pre-existing `ridl-lsp` test (the handshake
the tests perform is unchanged in shape).

- [ ] **Step 5: Gate and commit**

Run: `cargo fmt --all && cargo clippy --workspace --all-targets -- -D warnings`

```bash
git add crates/ridl/build.rs crates/ridl/src/main.rs crates/ridl-lsp/src/server.rs crates/ridl/tests/version.rs crates/ridl-lsp/tests/server.rs
git commit -m "feat(ridl): stamp the build version from RIDL_BUILD_VERSION and report it from ridl lsp

The editor-v release train ships the binary while the crates stay at 0.0.0;
without a stamped version a bug report from a Marketplace user names nothing.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 2: The extension's binary resolution module and test runner

**Files:**

- Create: `editors/vscode/src/binaryResolution.ts`,
  `editors/vscode/src/binaryResolution.test.ts`
- Modify: `editors/vscode/package.json` (`scripts.test`, the `ridl.serverPath`
  description)
- Modify: `editors/vscode/src/extension.ts` (use the module; spawn `ridl lsp`)

**Interfaces:**

- Consumes: `editors/vscode/src/extension.ts`'s `resolveServerPath()`
  (replaced).
- Produces:
  - `export interface ResolveInput { readonly configuredPath: string | undefined; readonly extensionPath: string; readonly platform: NodeJS.Platform; readonly exists: (file: string) => boolean }`
  - `export type BinarySource = "setting" | "bundled" | "path"`
  - `export function bundledBinaryPath(extensionPath: string, platform: NodeJS.Platform): string`
  - `export function resolveBinary(input: ResolveInput): { command: string; source: BinarySource }`
  - `export function isLegacyServerName(configuredPath: string): boolean` — true
    when the basename is `ridl-lsp` or `ridl-lsp.exe`.
  - `export const LSP_ARGS = ["lsp"] as const; export const MCP_ARGS = ["mcp"] as const;`

- [ ] **Step 1: Add the test runner and write the failing tests**

In `editors/vscode/package.json` `scripts`, add:

```json
"test": "npm run compile && node --test out/binaryResolution.test.js"
```

Create `editors/vscode/src/binaryResolution.test.ts`:

```ts
import { strict as assert } from "node:assert";
import * as path from "node:path";
import { test } from "node:test";
import {
  bundledBinaryPath,
  isLegacyServerName,
  LSP_ARGS,
  MCP_ARGS,
  resolveBinary,
} from "./binaryResolution";

const EXT = "/fake/extensions/driftsys.ridl-vscode-0.1.0";

test("tier 1: an explicit setting wins even when a bundled binary exists", () => {
  const resolved = resolveBinary({
    configuredPath: "/opt/ridl/bin/ridl",
    extensionPath: EXT,
    platform: "linux",
    exists: () => true,
  });
  assert.deepEqual(resolved, { command: "/opt/ridl/bin/ridl", source: "setting" });
});

test("tier 2: the bundled binary when it exists and no setting is given", () => {
  const bundled = path.join(EXT, "bin", "ridl");
  const resolved = resolveBinary({
    configuredPath: undefined,
    extensionPath: EXT,
    platform: "darwin",
    exists: (file) => file === bundled,
  });
  assert.deepEqual(resolved, { command: bundled, source: "bundled" });
});

test("tier 2 on Windows looks for ridl.exe", () => {
  assert.equal(bundledBinaryPath(EXT, "win32"), path.join(EXT, "bin", "ridl.exe"));
});

test("tier 3: PATH lookup when nothing is bundled (the development launch)", () => {
  const resolved = resolveBinary({
    configuredPath: undefined,
    extensionPath: EXT,
    platform: "linux",
    exists: () => false,
  });
  assert.deepEqual(resolved, { command: "ridl", source: "path" });
});

test("a blank setting counts as unset", () => {
  const resolved = resolveBinary({
    configuredPath: "   ",
    extensionPath: EXT,
    platform: "linux",
    exists: () => false,
  });
  assert.equal(resolved.source, "path");
});

test("the old ridl-lsp binary name is recognised so the user gets a warning", () => {
  assert.equal(isLegacyServerName("/home/dev/.cargo/bin/ridl-lsp"), true);
  assert.equal(isLegacyServerName("C:\\Users\\dev\\.cargo\\bin\\ridl-lsp.exe"), true);
  assert.equal(isLegacyServerName("/home/dev/.cargo/bin/ridl"), false);
});

test("the two roles are spawned with their subcommands", () => {
  assert.deepEqual([...LSP_ARGS], ["lsp"]);
  assert.deepEqual([...MCP_ARGS], ["mcp"]);
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd editors/vscode && npm ci && npm test` Expected: `tsc` fails —
`./binaryResolution` does not exist.

- [ ] **Step 3: Write the module and wire the LSP client to it**

Create `editors/vscode/src/binaryResolution.ts`:

```ts
// Resolves the one `ridl` binary the extension spawns in two roles. Pure —
// no `vscode` import — so it is unit-tested directly.
//
// Three tiers, first match wins:
//   1. the `ridl.serverPath` setting (a developer override),
//   2. the binary bundled in the extension at `bin/ridl(.exe)`,
//   3. `ridl` resolved from PATH — reached only when nothing is bundled,
//      which is the unpackaged extension-development launch.

import * as path from "node:path";

export interface ResolveInput {
  readonly configuredPath: string | undefined;
  readonly extensionPath: string;
  readonly platform: NodeJS.Platform;
  readonly exists: (file: string) => boolean;
}

export type BinarySource = "setting" | "bundled" | "path";

export const BINARY_NAME = "ridl";
export const LSP_ARGS = ["lsp"] as const;
export const MCP_ARGS = ["mcp"] as const;

export function bundledBinaryPath(extensionPath: string, platform: NodeJS.Platform): string {
  const file = platform === "win32" ? `${BINARY_NAME}.exe` : BINARY_NAME;
  return path.join(extensionPath, "bin", file);
}

export function resolveBinary(input: ResolveInput): { command: string; source: BinarySource } {
  const configured = input.configuredPath?.trim();
  if (configured) {
    return { command: configured, source: "setting" };
  }
  const bundled = bundledBinaryPath(input.extensionPath, input.platform);
  if (input.exists(bundled)) {
    return { command: bundled, source: "bundled" };
  }
  return { command: BINARY_NAME, source: "path" };
}

/** The setting used to name the `ridl-lsp` binary; that binary no longer exists. */
export function isLegacyServerName(configuredPath: string): boolean {
  const base = path.win32.basename(path.posix.basename(configuredPath));
  return base === "ridl-lsp" || base === "ridl-lsp.exe";
}
```

In `editors/vscode/src/extension.ts`:

- add `import * as fs from "node:fs";` and
  `import { isLegacyServerName, LSP_ARGS, resolveBinary } from "./binaryResolution";`
- replace `resolveServerPath()` and its use in `createClient()` with:

```ts
/** The `ridl.serverPath` setting, trimmed, or undefined when blank. */
function configuredServerPath(): string | undefined {
  const value = vscode.workspace.getConfiguration("ridl").get<string>("serverPath");
  return value && value.trim().length > 0 ? value.trim() : undefined;
}

function createClient(context: vscode.ExtensionContext): LanguageClient {
  const { command } = resolveBinary({
    configuredPath: configuredServerPath(),
    extensionPath: context.extensionPath,
    platform: process.platform,
    exists: (file) => fs.existsSync(file),
  });
  const serverOptions: ServerOptions = {
    command,
    args: [...LSP_ARGS],
    transport: TransportKind.stdio,
  };
  // ... clientOptions and the LanguageClient construction are unchanged.
}
```

- `activate` and `restartClient` pass `context` to `createClient` (store
  `context` in a module-level
  `let extensionContext: vscode.ExtensionContext | undefined;` set in
  `activate`, so `restartClient` can reach it).
- at the top of `activate`, add the one-time warning:

```ts
const configured = configuredServerPath();
if (configured && isLegacyServerName(configured)) {
  void vscode.window.showWarningMessage(
    "ridl.serverPath names the old ridl-lsp binary. The setting now points at the ridl binary, which the extension runs as `ridl lsp`.",
  );
}
```

- update the module doc comment at the top of `extension.ts` to describe the
  three tiers and the two roles.

In `editors/vscode/package.json`, change the `ridl.serverPath` description to:
`"Path to the ridl binary (run as`ridl lsp`and`ridl
mcp`). Empty (the default) uses the binary bundled in the extension, or`ridl`on PATH when none is bundled."`

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd editors/vscode && npm test` Expected: 7 passed; `tsc` clean.

- [ ] **Step 5: Try it in VS Code**

Run `cargo build -p ridl` (the plan's Task 1 binary) and open `editors/vscode`
in VS Code, press F5 (the extension-development host), open a `.ridl` file from
`docs/book/` in the host. Expected: diagnostics appear (tier 3, `ridl` from
`target/debug` must be on `PATH` for this —
`export PATH="$PWD/target/debug:$PATH"` before launching `code`). Then set
`ridl.serverPath` to `/…/target/debug/ridl` and confirm the client restarts
(tier 1).

- [ ] **Step 6: Commit**

```bash
git add editors/vscode/package.json editors/vscode/src/binaryResolution.ts editors/vscode/src/binaryResolution.test.ts editors/vscode/src/extension.ts
git commit -m "feat(editors): resolve one ridl binary in three tiers and spawn it as ridl lsp

Setting, then the bundled bin/ridl, then PATH. The PATH tier keeps the
unpackaged development launch working after cargo install --path crates/ridl.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 3: MCP registration and the manifest floor

**Files:**

- Modify: `editors/vscode/package.json` (`engines.vscode`,
  `devDependencies["@types/vscode"]`,
  `contributes.mcpServerDefinitionProviders`)
- Modify: `editors/vscode/package-lock.json` (via `npm install`)
- Modify: `editors/vscode/src/extension.ts` (`registerMcpProvider`)
- Modify: `editors/vscode/src/binaryResolution.ts` and `.test.ts`
  (`resolveMcpDefinition`)

**Interfaces:**

- Consumes: `resolveBinary`, `MCP_ARGS` (Task 2);
  `vscode.lm.registerMcpServerDefinitionProvider`,
  `vscode.McpStdioServerDefinition` (API 1.101).
- Produces:
  `export interface McpDefinitionInput extends ResolveInput { readonly extensionVersion: string; readonly workspaceFolder: string | undefined }`;
  `export function resolveMcpDefinition(input: McpDefinitionInput): { label: string; command: string; args: string[]; cwd: string | undefined; version: string }`;
  the provider id `ridl`.

- [ ] **Step 1: Write the failing test**

Append to `editors/vscode/src/binaryResolution.test.ts`:

```ts
import { resolveMcpDefinition } from "./binaryResolution";

test("the MCP definition spawns the same binary with the mcp subcommand", () => {
  const bundled = path.join(EXT, "bin", "ridl");
  const definition = resolveMcpDefinition({
    configuredPath: undefined,
    extensionPath: EXT,
    platform: "linux",
    exists: (file) => file === bundled,
    extensionVersion: "0.1.0",
    workspaceFolder: "/work/project",
  });
  assert.deepEqual(definition, {
    label: "RIDL",
    command: bundled,
    args: ["mcp"],
    cwd: "/work/project",
    version: "0.1.0",
  });
});
```

(Move the import to the top of the file with the others.)

- [ ] **Step 2: Run the test to verify it fails**

Run: `cd editors/vscode && npm test` Expected: `tsc` fails —
`resolveMcpDefinition` is not exported.

- [ ] **Step 3: Raise the API floor, declare the provider, and register it**

Run in `editors/vscode`:

```bash
npm install --save-dev --save-exact @types/vscode@1.101.0
```

Edit `package.json`: `"engines": { "vscode": "^1.101.0" }`, and inside
`contributes` add:

```json
"mcpServerDefinitionProviders": [
  { "id": "ridl", "label": "RIDL" }
]
```

Append to `binaryResolution.ts`:

```ts
export interface McpDefinitionInput extends ResolveInput {
  readonly extensionVersion: string;
  readonly workspaceFolder: string | undefined;
}

/** The MCP server definition VS Code hands to Copilot: `ridl mcp`, same binary as the LSP. */
export function resolveMcpDefinition(input: McpDefinitionInput): {
  label: string;
  command: string;
  args: string[];
  cwd: string | undefined;
  version: string;
} {
  const { command } = resolveBinary(input);
  return {
    label: "RIDL",
    command,
    args: [...MCP_ARGS],
    cwd: input.workspaceFolder,
    version: input.extensionVersion,
  };
}
```

In `extension.ts`, import `resolveMcpDefinition` and add:

```ts
const MCP_PROVIDER_ID = "ridl";

/**
 * Registers `ridl mcp` with VS Code's MCP registry so Copilot (and any other
 * MCP-aware client inside VS Code) sees the same binary the LSP uses. Claude
 * Code and Codex do not read this registry; crates/ridl-mcp/README.md covers
 * them.
 */
function registerMcpProvider(context: vscode.ExtensionContext): void {
  const didChange = new vscode.EventEmitter<void>();
  context.subscriptions.push(didChange);
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration((event) => {
      if (event.affectsConfiguration("ridl.serverPath")) {
        didChange.fire();
      }
    }),
  );
  context.subscriptions.push(
    vscode.lm.registerMcpServerDefinitionProvider(MCP_PROVIDER_ID, {
      onDidChangeMcpServerDefinitions: didChange.event,
      provideMcpServerDefinitions: () => {
        const resolved = resolveMcpDefinition({
          configuredPath: configuredServerPath(),
          extensionPath: context.extensionPath,
          platform: process.platform,
          exists: (file) => fs.existsSync(file),
          extensionVersion: String(context.extension.packageJSON.version),
          workspaceFolder: vscode.workspace.workspaceFolders?.[0]?.uri.fsPath,
        });
        const definition = new vscode.McpStdioServerDefinition(
          resolved.label,
          resolved.command,
          resolved.args,
          {},
          resolved.version,
        );
        if (resolved.cwd) {
          definition.cwd = vscode.Uri.file(resolved.cwd);
        }
        return [definition];
      },
    }),
  );
}
```

Call `registerMcpProvider(context);` in `activate` before
`await client.start();`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd editors/vscode && npm test` Expected: 8 passed; `tsc` accepts
`vscode.lm.registerMcpServerDefinitionProvider` and `McpStdioServerDefinition`
(proof the floor is right).

- [ ] **Step 5: Try it in VS Code**

Reload the extension-development host (Task 2 step 5). Open the Command Palette
→ "MCP: List Servers"; expected: `RIDL` listed, and starting it runs `ridl mcp`.
In Copilot Chat's tools picker, `ridl_check` appears.

- [ ] **Step 6: Commit**

```bash
git add editors/vscode/package.json editors/vscode/package-lock.json editors/vscode/src/extension.ts editors/vscode/src/binaryResolution.ts editors/vscode/src/binaryResolution.test.ts
git commit -m "feat(editors): register ridl mcp with VS Code's MCP registry

Raises the VS Code floor to 1.101 for lm.registerMcpServerDefinitionProvider;
the fork consequence is recorded in the design (spec §3).

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 4: The "Install ridl to PATH" command with stale-copy detection

**Files:**

- Create: `editors/vscode/src/installToPath.ts`,
  `editors/vscode/src/installToPath.test.ts`
- Modify: `editors/vscode/package.json` (`contributes.commands`, `scripts.test`)
- Modify: `editors/vscode/src/extension.ts` (the command handler and the
  activation check)

**Interfaces:**

- Consumes: `bundledBinaryPath` (Task 2).
- Produces (pure, no `vscode` import):
  - `export interface InstallPlan { readonly source: string; readonly targetDir: string; readonly target: string; readonly binaryName: string; readonly chmod: boolean }`
  - `export function planInstall(input: { platform: NodeJS.Platform; homedir: string; extensionPath: string }): InstallPlan`
  - `export function isDirOnPath(targetDir: string, pathEnv: string | undefined, delimiter: string, platform: NodeJS.Platform): boolean`
  - `export function pathHint(targetDir: string, platform: NodeJS.Platform): string`
  - `export async function performCopy(plan: InstallPlan): Promise<void>`
  - `export function parseVersionOutput(stdout: string): string | undefined` —
    `"ridl editor-v0.1.0\n"` → `"editor-v0.1.0"`.
  - `export function copyIsStale(bundledVersion: string | undefined, installedVersion: string | undefined): boolean`
    — true only when both are known and differ.

- [ ] **Step 1: Write the failing tests**

Create `editors/vscode/src/installToPath.test.ts`:

```ts
import { strict as assert } from "node:assert";
import { promises as fs } from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { test } from "node:test";
import {
  copyIsStale,
  isDirOnPath,
  parseVersionOutput,
  pathHint,
  performCopy,
  planInstall,
} from "./installToPath";

const EXT = "/fake/extensions/driftsys.ridl-vscode-0.1.0";
const HOME = "/home/dev";

test("planInstall on Linux: ~/.local/bin/ridl, chmod", () => {
  const plan = planInstall({ platform: "linux", homedir: HOME, extensionPath: EXT });
  assert.equal(plan.binaryName, "ridl");
  assert.equal(plan.source, path.join(EXT, "bin", "ridl"));
  assert.equal(plan.targetDir, path.join(HOME, ".local", "bin"));
  assert.equal(plan.target, path.join(HOME, ".local", "bin", "ridl"));
  assert.equal(plan.chmod, true);
});

test("planInstall on Windows: ridl.exe, no chmod", () => {
  const plan = planInstall({ platform: "win32", homedir: "C:\\Users\\dev", extensionPath: EXT });
  assert.equal(plan.binaryName, "ridl.exe");
  assert.equal(plan.chmod, false);
});

test("isDirOnPath is exact on POSIX and case-insensitive on Windows", () => {
  assert.equal(isDirOnPath("/home/dev/.local/bin", "/usr/bin:/home/dev/.local/bin", ":", "linux"), true);
  assert.equal(isDirOnPath("/home/dev/.local/bin", "/usr/bin:/home/dev/.local/BIN", ":", "linux"), false);
  assert.equal(isDirOnPath("C:\\Users\\dev\\bin", "C:\\Windows;c:\\users\\DEV\\bin", ";", "win32"), true);
  assert.equal(isDirOnPath("/x", undefined, ":", "linux"), false);
});

test("pathHint prints a shell line per platform", () => {
  assert.equal(pathHint("/home/dev/.local/bin", "darwin"), 'export PATH="/home/dev/.local/bin:$PATH"');
  assert.match(pathHint("C:\\Users\\dev\\bin", "win32"), /Path/);
});

test("performCopy creates the directory, copies, and sets the mode", async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "ridl-install-"));
  const source = path.join(dir, "ridl");
  await fs.writeFile(source, "#!/bin/sh\necho ridl\n");
  const plan = {
    source,
    targetDir: path.join(dir, "target", "bin"),
    target: path.join(dir, "target", "bin", "ridl"),
    binaryName: "ridl",
    chmod: process.platform !== "win32",
  };
  await performCopy(plan);
  const copied = await fs.readFile(plan.target, "utf8");
  assert.equal(copied, "#!/bin/sh\necho ridl\n");
  if (plan.chmod) {
    const mode = (await fs.stat(plan.target)).mode & 0o777;
    assert.equal(mode, 0o755);
  }
});

test("parseVersionOutput takes the token after the program name", () => {
  assert.equal(parseVersionOutput("ridl editor-v0.1.0\n"), "editor-v0.1.0");
  assert.equal(parseVersionOutput("ridl 0.0.0"), "0.0.0");
  assert.equal(parseVersionOutput(""), undefined);
});

test("copyIsStale only when both versions are known and differ", () => {
  assert.equal(copyIsStale("editor-v0.2.0", "editor-v0.1.0"), true);
  assert.equal(copyIsStale("editor-v0.2.0", "editor-v0.2.0"), false);
  assert.equal(copyIsStale(undefined, "editor-v0.1.0"), false);
  assert.equal(copyIsStale("editor-v0.2.0", undefined), false);
});
```

Change `scripts.test` in `package.json` to run both files:

```json
"test": "npm run compile && node --test out/binaryResolution.test.js out/installToPath.test.js"
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cd editors/vscode && npm test` Expected: `tsc` fails — `./installToPath`
does not exist.

- [ ] **Step 3: Write the module, the command, and the activation check**

Create `editors/vscode/src/installToPath.ts`:

```ts
// The "Install ridl to PATH" command: copies the bundled binary to
// ~/.local/bin so Claude Code, Codex, and a terminal can run `ridl` without a
// second download. Pure planning and filesystem work — no `vscode` import;
// extension.ts supplies the notifications.

import { promises as fs } from "node:fs";
import * as path from "node:path";
import { bundledBinaryPath, BINARY_NAME } from "./binaryResolution";

export interface InstallPlan {
  readonly source: string;
  readonly targetDir: string;
  readonly target: string;
  readonly binaryName: string;
  readonly chmod: boolean;
}

export function planInstall(input: {
  platform: NodeJS.Platform;
  homedir: string;
  extensionPath: string;
}): InstallPlan {
  const binaryName = input.platform === "win32" ? `${BINARY_NAME}.exe` : BINARY_NAME;
  const targetDir = path.join(input.homedir, ".local", "bin");
  return {
    binaryName,
    source: bundledBinaryPath(input.extensionPath, input.platform),
    targetDir,
    target: path.join(targetDir, binaryName),
    chmod: input.platform !== "win32",
  };
}

export function isDirOnPath(
  targetDir: string,
  pathEnv: string | undefined,
  delimiter: string,
  platform: NodeJS.Platform,
): boolean {
  if (!pathEnv) return false;
  const norm = (entry: string) => (platform === "win32" ? entry.trim().toLowerCase() : entry.trim());
  return pathEnv.split(delimiter).map(norm).includes(norm(targetDir));
}

/** The line that adds `targetDir` to PATH. On Windows, through the user
 * environment in the registry — `setx` truncates a PATH longer than 1024
 * characters. */
export function pathHint(targetDir: string, platform: NodeJS.Platform): string {
  return platform === "win32"
    ? `[Environment]::SetEnvironmentVariable("Path", "${targetDir};" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")`
    : `export PATH="${targetDir}:$PATH"`;
}

export async function performCopy(plan: InstallPlan): Promise<void> {
  await fs.mkdir(plan.targetDir, { recursive: true });
  await fs.copyFile(plan.source, plan.target);
  if (plan.chmod) await fs.chmod(plan.target, 0o755);
}

/** `ridl --version` prints `ridl <version>`; returns `<version>`. */
export function parseVersionOutput(stdout: string): string | undefined {
  const parts = stdout.trim().split(/\s+/);
  return parts.length >= 2 ? parts[1] : undefined;
}

export function copyIsStale(
  bundledVersion: string | undefined,
  installedVersion: string | undefined,
): boolean {
  return bundledVersion !== undefined && installedVersion !== undefined && bundledVersion !== installedVersion;
}
```

In `package.json` `contributes`, add:

```json
"commands": [
  { "command": "ridl.installToPath", "title": "Install ridl to PATH", "category": "RIDL" }
]
```

In `extension.ts`, add imports (`node:child_process` `execFile`, `node:os`,
`node:util` `promisify`, and the new module) and:

```ts
const execFileAsync = promisify(execFile);

async function versionOf(binary: string): Promise<string | undefined> {
  try {
    const { stdout } = await execFileAsync(binary, ["--version"]);
    return parseVersionOutput(stdout);
  } catch {
    return undefined;
  }
}

async function installToPath(context: vscode.ExtensionContext): Promise<void> {
  const plan = planInstall({
    platform: process.platform,
    homedir: os.homedir(),
    extensionPath: context.extensionPath,
  });
  if (!fs.existsSync(plan.source)) {
    void vscode.window.showErrorMessage(
      "This extension build has no bundled ridl binary (a development launch). Run cargo install --path crates/ridl instead.",
    );
    return;
  }
  try {
    await performCopy(plan);
  } catch (error) {
    void vscode.window.showErrorMessage(`Could not copy ridl to ${plan.target}: ${String(error)}`);
    return;
  }
  const onPath = isDirOnPath(plan.targetDir, process.env.PATH ?? process.env.Path, path.delimiter, process.platform);
  if (onPath) {
    void vscode.window.showInformationMessage(`ridl installed to ${plan.target}`);
  } else {
    const hint = pathHint(plan.targetDir, process.platform);
    const pick = await vscode.window.showWarningMessage(
      `ridl copied to ${plan.target}, but ${plan.targetDir} is not on your PATH.`,
      "Copy PATH command",
    );
    if (pick === "Copy PATH command") {
      await vscode.env.clipboard.writeText(hint);
    }
  }
}

/** On activation: when a copy on PATH is older than the bundled binary, offer to re-copy. */
async function offerRefreshOfStaleCopy(context: vscode.ExtensionContext): Promise<void> {
  const plan = planInstall({ platform: process.platform, homedir: os.homedir(), extensionPath: context.extensionPath });
  if (!fs.existsSync(plan.source) || !fs.existsSync(plan.target)) return;
  const [bundled, installed] = await Promise.all([versionOf(plan.source), versionOf(plan.target)]);
  if (!copyIsStale(bundled, installed)) return;
  const pick = await vscode.window.showInformationMessage(
    `ridl on PATH is ${installed}; this extension bundles ${bundled}.`,
    "Update the copy",
  );
  if (pick === "Update the copy") {
    await installToPath(context);
  }
}
```

In `activate`:
`context.subscriptions.push(vscode.commands.registerCommand("ridl.installToPath", () => installToPath(context)));`
and `void offerRefreshOfStaleCopy(context);` (not awaited — activation must not
wait on two process spawns).

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cd editors/vscode && npm test` Expected: 15 passed.

- [ ] **Step 5: Try it in VS Code**

Package a local build (`just package-vscode` arrives in Task 5; for now:
`cargo build --release -p ridl && mkdir -p editors/vscode/bin && cp target/release/ridl editors/vscode/bin/ && cd editors/vscode && npx vsce package`),
install the `.vsix` with `code --install-extension`, run "RIDL: Install ridl to
PATH". Expected: `~/.local/bin/ridl --version` prints `ridl 0.0.0`. Then rebuild
with `RIDL_BUILD_VERSION=editor-v9.9.9 cargo build --release -p ridl`,
repackage, reinstall, reload: the "ridl on PATH is 0.0.0; this extension bundles
editor-v9.9.9" message appears.

- [ ] **Step 6: Commit**

```bash
git add editors/vscode/package.json editors/vscode/src/installToPath.ts editors/vscode/src/installToPath.test.ts editors/vscode/src/extension.ts
git commit -m "feat(editors): add the Install ridl to PATH command with stale-copy detection

Copies the bundled binary to ~/.local/bin for Claude Code, Codex, and the
terminal; on activation compares versions and offers to refresh an old copy.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 5: Repository hygiene, the `just` recipes, and the LSP binary removal

**Files:**

- Modify: `.gitignore` (add `editors/vscode/bin/`)
- Create: `editors/vscode/.vscodeignore`
- Modify: `justfile` (two recipes after `release`)
- Delete: `crates/ridl-lsp/src/main.rs`
- Modify: `crates/ridl-lsp/src/lib.rs:1-25` (module doc),
  `editors/vscode/README.md`, `CONTRIBUTING.md:45`, `README.md:49`,
  `docs/book/cli-reference.md:5`

**Interfaces:**

- Produces: `just vscode-verify` (no binary; `npm ci`, `npm test`,
  `vsce package` to a temp file, `vsce ls` check) and `just package-vscode`
  (host `ridl` release build, copy into `bin/`, `vsce package`).

- [ ] **Step 1: Hygiene files**

Append to `.gitignore` under the "VS Code extension build output" block:

```gitignore
editors/vscode/bin/
```

Create `editors/vscode/.vscodeignore`:

```gitignore
.vscode/**

# TypeScript sources and maps — VS Code runs the compiled out/*.js.
src/**
**/*.ts
**/*.map
tsconfig.json

# Compiled tests are inert at runtime.
out/**/*.test.js

# bin/ is NOT ignored: just package-vscode and the release workflow put the
# ridl binary there, and it must ship in the VSIX.

# Development-only dependency content.
node_modules/typescript/**
node_modules/@types/**
node_modules/@vscode/**
node_modules/**/*.md
node_modules/**/*.ts
node_modules/**/*.map
node_modules/**/test/**
node_modules/**/tests/**
node_modules/**/.bin/**

.gitignore
.vscodeignore
package-lock.json
```

- [ ] **Step 2: The recipes**

Append to `justfile` after the `release` recipe:

```just
# Verify the VS Code extension packages: compile, unit tests, and a `vsce
# package` with no bundled binary (a fresh checkout has no bin/, and the
# manifest does not reference it). Invoked by vscode-verify.yaml on pull
# requests that touch editors/vscode. Not a member of `build`, so gate-parity
# does not cover it; the workflow is the only caller besides a contributor.
vscode-verify:
    #!/usr/bin/env bash
    set -euo pipefail
    cd editors/vscode
    npm ci
    npm test
    out="$(mktemp -t ridl-vscode-verify.XXXXXX).vsix"
    npx vsce package --out "$out"
    if npx vsce ls | grep -q '^src/'; then
        echo "vscode-verify: src/ would ship in the VSIX — check .vscodeignore" >&2
        exit 1
    fi
    echo "vscode-verify: packaged $out"

# Build the extension for this machine: a release build of ridl copied into
# editors/vscode/bin/, then `vsce package`. Local testing only — the release
# workflow runs the same commands per target with --target. Not a member of
# `build`.
package-vscode:
    #!/usr/bin/env bash
    set -euo pipefail
    cargo build --release --locked -p ridl
    bin=target/release/ridl
    if [ -f target/release/ridl.exe ]; then bin=target/release/ridl.exe; fi
    mkdir -p editors/vscode/bin
    cp "$bin" editors/vscode/bin/
    cd editors/vscode
    npm ci
    npm run compile
    npx vsce package
```

- [ ] **Step 3: Remove the `ridl-lsp` binary and update the documentation**

```bash
git rm crates/ridl-lsp/src/main.rs
```

In `crates/ridl-lsp/src/lib.rs`'s module doc, replace the sentence describing
the binary with: "The server is a library; the `ridl` CLI hosts it as
`ridl lsp`, which the VS Code extension and the release artifacts use."

In `editors/vscode/README.md`: replace the "Prerequisites" and "Build the
extension" sections with:

````markdown
## Install

From the VS Code Marketplace or Open VSX, search "RIDL". The extension
bundles the `ridl` binary and runs it as `ridl lsp` (the language server) and
`ridl mcp` (the MCP server Copilot sees). Nothing else to install.

To use `ridl` from a terminal, Claude Code, or Codex as well, run the
command **RIDL: Install ridl to PATH**, or use the installer script from the
repository root (`install.sh` / `install.ps1`).

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
````

and update the settings table row for `ridl.serverPath` to: "Path to the `ridl`
binary. Empty (the default) uses the bundled binary, or `ridl` on `PATH` when
none is bundled (a development launch)." Replace every remaining `ridl-lsp` in
that README with `ridl lsp`. Update `CONTRIBUTING.md:45` (the crate list —
`ridl-lsp` stays, it is still a crate) only if it names the binary; update
`README.md:49` to "Language server library (`ridl lsp`)"; update
`docs/book/cli-reference.md:5` to describe `ridl lsp` rather than a separate
`ridl-lsp` binary.

- [ ] **Step 4: Verify**

Run:
`just vscode-verify && just package-vscode && ls editors/vscode/*.vsix && cd editors/vscode && npx vsce ls | grep -c '^bin/ridl'`
Expected: both recipes succeed; the `.vsix` exists; the count is `1`. Then
`cargo test -p ridl-lsp` (the library tests still pass without `main.rs`) and
`just build`.

- [ ] **Step 5: Commit**

```bash
git add .gitignore editors/vscode/.vscodeignore justfile crates/ridl-lsp/src/lib.rs editors/vscode/README.md CONTRIBUTING.md README.md docs/book/cli-reference.md
git commit -m "chore(repo): add the extension packaging recipes, .vscodeignore, and retire the ridl-lsp binary

just vscode-verify packages without a binary; just package-vscode bundles a
host build. The extension now spawns ridl lsp, so the separate binary goes.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 6: The verify workflow

**Files:**

- Create: `.github/workflows/vscode-verify.yaml`

**Interfaces:**

- Consumes: `just vscode-verify` (Task 5).

- [ ] **Step 1: Write the workflow**

```yaml
name: vscode-verify

# Packages the VS Code extension on every pull request that touches it, with no
# bundled binary and no credentials: a manifest or .vscodeignore mistake fails
# here instead of in a release. The gate command is `just vscode-verify`
# (ADR-0009: the justfile defines it; this file only invokes it). It is not a
# member of `just build`, so `just gate-parity` does not cover this workflow.

on:
  pull_request:
    paths:
      - "editors/vscode/**"
      - ".github/workflows/vscode-verify.yaml"
      - ".github/workflows/vscode-release.yaml"
      - "justfile"

permissions:
  contents: read

env:
  JUST_VERSION: 1.38.0

jobs:
  verify:
    name: verify
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: install just
        run: |
          mkdir -p "$HOME/.local/bin"
          curl -fsSL "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
            | tar -xz -C "$HOME/.local/bin" just
          echo "$HOME/.local/bin" >> "$GITHUB_PATH"
      - uses: actions/setup-node@v4
        with:
          node-version: 20
      - run: just vscode-verify
```

- [ ] **Step 2: Verify the file parses and the parity gate still passes**

Run:
`ruby -ryaml -e 'YAML.load_file(".github/workflows/vscode-verify.yaml"); puts "ok"' && just gate-parity && just fmt && just check`
Expected: `ok`; gate-parity passes (it reads `ci.yml` only); prim leaves the
file as written.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/vscode-verify.yaml
git commit -m "ci(ci): package the VS Code extension on pull requests that touch it

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 7: The release workflow

**Files:**

- Create: `.github/workflows/vscode-release.yaml`
- Modify: `editors/vscode/package.json` (`devDependencies.ovsx`)

**Interfaces:**

- Consumes: `RIDL_BUILD_VERSION` (Task 1); the extension manifest (Tasks 2–4);
  `.vscodeignore` (Task 5).
- Produces: a GitHub Release per `editor-v*` tag carrying
  `ridl-<target>.tar.gz`, `.sha256`, and `ridl-vscode-<vsce-target>.vsix` for
  the five targets; two environment-gated publish jobs.

- [ ] **Step 1: Add `ovsx` and write the workflow**

Run in `editors/vscode`: `npm install --save-dev --save-exact ovsx@0.10.5` (or
the current 0.10.x).

Create `.github/workflows/vscode-release.yaml`:

```yaml
name: vscode-release

# Release train for the ridl binary and the VS Code extension, triggered by an
# `editor-v*` tag a maintainer pushes (ADR-0007 decision 14: the push is the
# maintainer act). Builds ridl for five targets with the tag stamped as its
# version, packages one VSIX per target, creates the GitHub Release, and holds
# the Marketplace and Open VSX publishes behind the `marketplace` environment's
# required reviewers. The release job does not depend on the publish jobs, so
# a publish failure never blocks the release.
#
# Neither `vsce publish` nor `ovsx publish` is idempotent: after a partial
# failure, publish the remaining targets by hand rather than re-running.

on:
  push:
    tags: ["editor-v*"]

permissions:
  contents: write

env:
  NODE_VERSION: 20

jobs:
  guard:
    name: tag matches package.json
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: compare versions
        run: |
          tag="${GITHUB_REF_NAME#editor-v}"
          manifest="$(node -p "require('./editors/vscode/package.json').version")"
          if [ "$tag" != "$manifest" ]; then
            echo "::error::tag version $tag does not match editors/vscode/package.json version $manifest" >&2
            exit 1
          fi

  build:
    name: build (${{ matrix.target }})
    needs: guard
    runs-on: ${{ matrix.os }}
    defaults:
      run:
        shell: bash
    strategy:
      fail-fast: false
      matrix:
        include:
          - { target: x86_64-unknown-linux-gnu, os: ubuntu-latest, binary: ridl }
          - { target: aarch64-unknown-linux-gnu, os: ubuntu-latest, binary: ridl }
          - { target: x86_64-apple-darwin, os: macos-latest, binary: ridl }
          - { target: aarch64-apple-darwin, os: macos-latest, binary: ridl }
          - { target: x86_64-pc-windows-msvc, os: windows-latest, binary: ridl.exe }
    steps:
      - uses: actions/checkout@v4
      - name: add the target
        run: rustup target add ${{ matrix.target }}
      - name: cross toolchain (aarch64 linux)
        if: matrix.target == 'aarch64-unknown-linux-gnu'
        uses: taiki-e/setup-cross-toolchain-action@v1
        with:
          target: ${{ matrix.target }}
      - name: build ridl
        env:
          RIDL_BUILD_VERSION: ${{ github.ref_name }}
        run: cargo build --release --locked --target ${{ matrix.target }} -p ridl
      - name: tarball and checksum
        run: |
          out="target/${{ matrix.target }}/release/${{ matrix.binary }}"
          tar czf "ridl-${{ matrix.target }}.tar.gz" -C "$(dirname "$out")" "${{ matrix.binary }}"
          if command -v sha256sum >/dev/null; then
            sha256sum "ridl-${{ matrix.target }}.tar.gz" > "ridl-${{ matrix.target }}.tar.gz.sha256"
          else
            shasum -a 256 "ridl-${{ matrix.target }}.tar.gz" > "ridl-${{ matrix.target }}.tar.gz.sha256"
          fi
      - uses: actions/upload-artifact@v4
        with:
          name: ridl-${{ matrix.target }}
          path: |
            ridl-${{ matrix.target }}.tar.gz
            ridl-${{ matrix.target }}.tar.gz.sha256

  package-vsix:
    name: package vsix (${{ matrix.vsce-target }})
    needs: build
    runs-on: ubuntu-latest
    strategy:
      fail-fast: false
      matrix:
        include:
          - { target: x86_64-unknown-linux-gnu, vsce-target: linux-x64, binary: ridl }
          - { target: aarch64-unknown-linux-gnu, vsce-target: linux-arm64, binary: ridl }
          - { target: x86_64-apple-darwin, vsce-target: darwin-x64, binary: ridl }
          - { target: aarch64-apple-darwin, vsce-target: darwin-arm64, binary: ridl }
          - { target: x86_64-pc-windows-msvc, vsce-target: win32-x64, binary: ridl.exe }
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: ${{ env.NODE_VERSION }}
      - uses: actions/download-artifact@v4
        with:
          name: ridl-${{ matrix.target }}
          path: artifacts
      - name: bundle the binary
        run: |
          mkdir -p editors/vscode/bin
          tar xzf "artifacts/ridl-${{ matrix.target }}.tar.gz" -C editors/vscode/bin
          chmod +x "editors/vscode/bin/${{ matrix.binary }}"
      - name: package
        run: |
          cd editors/vscode
          npm ci
          npm run compile
          npx vsce package --target ${{ matrix.vsce-target }} --out "ridl-vscode-${{ matrix.vsce-target }}.vsix"
      - uses: actions/upload-artifact@v4
        with:
          name: ridl-vscode-${{ matrix.vsce-target }}-vsix
          path: editors/vscode/ridl-vscode-${{ matrix.vsce-target }}.vsix

  release:
    name: github release
    needs: [build, package-vsix]
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/download-artifact@v4
        with:
          path: artifacts
          merge-multiple: true
      - uses: softprops/action-gh-release@v2
        with:
          tag_name: ${{ github.ref_name }}
          generate_release_notes: true
          # This train must not become the repository's "latest": install.sh
          # filters by tag prefix, and a workspace release keeps that slot.
          make_latest: false
          files: |
            artifacts/*.tar.gz
            artifacts/*.sha256
            artifacts/*.vsix

  publish-vsce:
    name: publish to the VS Code Marketplace
    needs: package-vsix
    runs-on: ubuntu-latest
    environment: marketplace
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: ${{ env.NODE_VERSION }}
      - uses: actions/download-artifact@v4
        with:
          path: vsix
          pattern: ridl-vscode-*-vsix
          merge-multiple: true
      - name: publish each target
        env:
          VSCE_PAT: ${{ secrets.VSCE_PAT }}
        run: |
          shopt -s nullglob
          files=(vsix/*.vsix)
          if [ "${#files[@]}" -ne 5 ]; then
            echo "::error::expected 5 VSIX files, found ${#files[@]}" >&2
            exit 1
          fi
          for f in "${files[@]}"; do
            echo "publishing $f"
            npx -y @vscode/vsce publish --packagePath "$f" --pat "$VSCE_PAT"
          done

  publish-ovsx:
    name: publish to Open VSX
    needs: package-vsix
    runs-on: ubuntu-latest
    environment: marketplace
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with:
          node-version: ${{ env.NODE_VERSION }}
      - uses: actions/download-artifact@v4
        with:
          path: vsix
          pattern: ridl-vscode-*-vsix
          merge-multiple: true
      - name: publish each target
        env:
          OVSX_PAT: ${{ secrets.OVSX_PAT }}
        run: |
          shopt -s nullglob
          files=(vsix/*.vsix)
          if [ "${#files[@]}" -ne 5 ]; then
            echo "::error::expected 5 VSIX files, found ${#files[@]}" >&2
            exit 1
          fi
          for f in "${files[@]}"; do
            echo "publishing $f"
            npx -y ovsx publish "$f" --pat "$OVSX_PAT"
          done
```

- [ ] **Step 2: Verify what can be verified locally**

Run:
`ruby -ryaml -e 'YAML.load_file(".github/workflows/vscode-release.yaml"); puts "ok"' && just gate-parity && just fmt && just check`
Expected: `ok`, gate-parity passes, prim clean. Then exercise the per-target
packaging command by hand for the host:
`cd editors/vscode && npx vsce package --target darwin-arm64 --out /tmp/t.vsix`
(after `just package-vscode` put a binary in `bin/`) — expected: a `.vsix` whose
`npx vsce ls` includes `bin/ridl`.

Running the workflow end to end requires a pushed tag and the `marketplace`
environment — maintainer acts, listed in Task 9.

- [ ] **Step 3: Commit**

```bash
git add .github/workflows/vscode-release.yaml editors/vscode/package.json editors/vscode/package-lock.json
git commit -m "ci(ci): add the editor-v release train for ridl and the VS Code extension

Five targets, tarballs and per-target VSIX files on a GitHub Release, and
Marketplace and Open VSX publishes held behind the marketplace environment.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 8: `install.sh` and `install.ps1`

**Files:**

- Create: `install.sh`, `install.ps1`
- Test: `tests/install_sh.sh` is not a convention here; verification is a dry
  run (below).

**Interfaces:**

- Produces: `install.sh` honoring `RIDL_VERSION` (an `editor-v*` tag),
  `RIDL_INSTALL_DIR` (default `$HOME/.local/bin`), and `RIDL_INSTALL_DRY_RUN=1`
  (prints the resolved URL and exits 0 without downloading); `install.ps1` with
  the same three as `$env:` variables.

- [ ] **Step 1: Write `install.sh`**

```bash
#!/usr/bin/env bash
# Installs the ridl binary from the newest editor-v* GitHub Release (or the one
# named in RIDL_VERSION): downloads ridl-<target>.tar.gz and its .sha256,
# verifies the checksum, and places `ridl` in RIDL_INSTALL_DIR (default
# ~/.local/bin). Usage:
#   curl -fsSL https://raw.githubusercontent.com/driftsys/ridl/main/install.sh | bash
set -eu

REPO="driftsys/ridl"
TAG_PREFIX="editor-v"
INSTALL_DIR="${RIDL_INSTALL_DIR:-$HOME/.local/bin}"
BINARY="ridl"

detect_target() {
  case "$(uname -s)/$(uname -m)" in
    Linux/x86_64)   echo "x86_64-unknown-linux-gnu" ;;
    Linux/aarch64)  echo "aarch64-unknown-linux-gnu" ;;
    Darwin/x86_64)  echo "x86_64-apple-darwin" ;;
    Darwin/arm64)   echo "aarch64-apple-darwin" ;;
    *) echo "error: unsupported platform: $(uname -s)/$(uname -m)" >&2; exit 1 ;;
  esac
}

# The newest release whose tag starts with editor-v. Never /releases/latest:
# that is the newest release across every tag, and the workspace's own v*
# releases must not be picked up here.
get_version() {
  if [ -n "${RIDL_VERSION:-}" ]; then
    printf '%s\n' "$RIDL_VERSION"
    return
  fi
  if command -v gh >/dev/null 2>&1; then
    if version=$(gh release list --repo "$REPO" --limit 100 --json tagName \
        -q "[.[].tagName | select(startswith(\"$TAG_PREFIX\"))][0]" 2>/dev/null) \
       && [ -n "$version" ]; then
      printf '%s\n' "$version"
      return
    fi
  fi
  body=$(curl -fsSL "https://api.github.com/repos/$REPO/releases?per_page=100") \
    || { echo "error: could not reach the GitHub API" >&2; exit 1; }
  version=$(printf '%s\n' "$body" | grep '"tag_name"' | cut -d'"' -f4 | grep "^$TAG_PREFIX" | head -1)
  if [ -z "$version" ]; then
    echo "error: no $TAG_PREFIX* release found" >&2
    exit 1
  fi
  printf '%s\n' "$version"
}

main() {
  local target version tarball url
  target="$(detect_target)"
  version="$(get_version)"
  tarball="ridl-${target}.tar.gz"
  url="https://github.com/$REPO/releases/download/$version/$tarball"

  if [ -n "${RIDL_INSTALL_DRY_RUN:-}" ]; then
    echo "$url"
    return
  fi

  echo "Installing ridl $version ($target) to $INSTALL_DIR" >&2
  tmpdir="$(mktemp -d)"
  trap 'rm -rf "${tmpdir:-}"' EXIT
  curl -fsSL "$url" -o "$tmpdir/$tarball"
  curl -fsSL "$url.sha256" -o "$tmpdir/$tarball.sha256"
  (cd "$tmpdir" && if command -v sha256sum >/dev/null; then sha256sum -c "$tarball.sha256"; else shasum -a 256 -c "$tarball.sha256"; fi)
  tar xzf "$tmpdir/$tarball" -C "$tmpdir"
  mkdir -p "$INSTALL_DIR"
  mv "$tmpdir/$BINARY" "$INSTALL_DIR/$BINARY"
  chmod +x "$INSTALL_DIR/$BINARY"
  echo "Installed $INSTALL_DIR/$BINARY ($("$INSTALL_DIR/$BINARY" --version))" >&2
  if ! printf '%s\n' "$PATH" | tr ':' '\n' | grep -qx "$INSTALL_DIR"; then
    echo "" >&2
    echo "Add to your PATH:" >&2
    echo "  export PATH=\"$INSTALL_DIR:\$PATH\"" >&2
  fi
}

main
```

`chmod +x install.sh`.

- [ ] **Step 2: Write `install.ps1`**

```powershell
<#
.SYNOPSIS
    Installs the ridl binary on Windows from the newest editor-v* GitHub
    Release (or $env:RIDL_VERSION): downloads ridl-x86_64-pc-windows-msvc.tar.gz
    and its .sha256, verifies the checksum, and places ridl.exe in
    $env:RIDL_INSTALL_DIR (default $HOME\.local\bin).
.EXAMPLE
    irm https://raw.githubusercontent.com/driftsys/ridl/main/install.ps1 | iex
#>
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'

$Repo = 'driftsys/ridl'
$TagPrefix = 'editor-v'
$Target = 'x86_64-pc-windows-msvc'
$Binary = 'ridl.exe'
$InstallDir = if ($env:RIDL_INSTALL_DIR) { $env:RIDL_INSTALL_DIR } else { Join-Path $HOME '.local\bin' }

function Get-Version {
    if ($env:RIDL_VERSION) { return $env:RIDL_VERSION }
    # Never /releases/latest: it is the newest release across every tag.
    $releases = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases?per_page=100" -UseBasicParsing
    $match = $releases | Where-Object { $_.tag_name -like "$TagPrefix*" } | Select-Object -First 1
    if (-not $match) { Write-Error "no $TagPrefix* release found" }
    return $match.tag_name
}

function Test-Checksum {
    param([string]$File, [string]$ChecksumFile)
    $expected = ((Get-Content -LiteralPath $ChecksumFile -Raw).Trim() -split '\s+', 2)[0]
    $actual = (Get-FileHash -LiteralPath $File -Algorithm SHA256).Hash
    if ($expected.ToLower() -ne $actual.ToLower()) {
        Write-Error "checksum mismatch: expected $expected, got $actual"
    }
}

function Main {
    $version = Get-Version
    $tarball = "ridl-$Target.tar.gz"
    $url = "https://github.com/$Repo/releases/download/$version/$tarball"
    if ($env:RIDL_INSTALL_DRY_RUN) { Write-Output $url; return }

    Write-Host "Installing ridl $version ($Target) to $InstallDir"
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("ridl-install-" + [System.Guid]::NewGuid())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        Invoke-WebRequest -Uri $url -OutFile (Join-Path $tmp $tarball) -UseBasicParsing
        Invoke-WebRequest -Uri "$url.sha256" -OutFile (Join-Path $tmp "$tarball.sha256") -UseBasicParsing
        Test-Checksum -File (Join-Path $tmp $tarball) -ChecksumFile (Join-Path $tmp "$tarball.sha256")
        # Windows 10 1803+ ships bsdtar as tar.exe.
        tar -xzf (Join-Path $tmp $tarball) -C $tmp
        New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
        Move-Item -LiteralPath (Join-Path $tmp $Binary) -Destination (Join-Path $InstallDir $Binary) -Force
        Write-Host "Installed $(Join-Path $InstallDir $Binary)"
        $onPath = ($env:Path -split ';') | Where-Object { $_.Trim().ToLower() -eq $InstallDir.ToLower() }
        if (-not $onPath) {
            # Through the registry, not setx: setx truncates a PATH over 1024 characters.
            Write-Host ""
            Write-Host "Add to your PATH (PowerShell):"
            Write-Host "  [Environment]::SetEnvironmentVariable('Path', `"$InstallDir;`" + [Environment]::GetEnvironmentVariable('Path', 'User'), 'User')"
        }
    } finally {
        Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue
    }
}

Main
```

- [ ] **Step 3: Verify**

Run:

```bash
bash -n install.sh
RIDL_VERSION=editor-v0.1.0 RIDL_INSTALL_DRY_RUN=1 bash install.sh
command -v shellcheck >/dev/null && shellcheck install.sh || echo "shellcheck not installed — skipped"
command -v pwsh >/dev/null && pwsh -NoProfile -Command '$env:RIDL_VERSION="editor-v0.1.0"; $env:RIDL_INSTALL_DRY_RUN="1"; ./install.ps1' || echo "pwsh not installed — skipped"
```

Expected: the dry run prints
`https://github.com/driftsys/ridl/releases/download/editor-v0.1.0/ridl-<host target>.tar.gz`
(and the PowerShell one the `x86_64-pc-windows-msvc` URL).

- [ ] **Step 4: Commit**

```bash
git add install.sh install.ps1
git commit -m "feat(repo): add install.sh and install.ps1 for the ridl binary

Both resolve the newest editor-v* release by tag prefix, verify the sha256,
and install to ~/.local/bin — the path Claude Code and Codex users take.

Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>"
```

---

### Task 9: The ADR amendment, the maintainer checklist, and the hand-off

**Files:**

- Modify: `docs/decisions/ADR-0007-e1-execution.md` (an amendment paragraph
  after decision 14)
- Modify: `AGENTS.md` (the `Commands` list: `vscode-verify`, `package-vscode`)

- [ ] **Step 1: Record the second release train**

After decision 14 in ADR-0007 (line ~147), add, in the file's existing amendment
style (see ADR-0005's "_Corrected (2026-07-26)._" paragraph):

```markdown
_Amended (2026-09-XX)._ A second release train exists beside this decision:
an `editor-v<version>` tag, pushed by a maintainer, builds the `ridl` binary
for five targets and the VS Code extension, publishes a GitHub Release, and
holds the Marketplace and Open VSX publishes behind a reviewed `marketplace`
environment (`.github/workflows/vscode-release.yaml`). The workspace crates
stay at `0.0.0`; the binary reports the tag through `RIDL_BUILD_VERSION`.
The tag push and the environment approval are the maintainer acts this
decision requires. Design: `docs/wip/2026-09-13-vscode-extension-
distribution-design.md` (gardened with this branch).
```

Replace `2026-09-XX` with the day the branch is finished. Add the two recipes to
`AGENTS.md`'s `Commands` block with one-line descriptions matching the justfile
comments.

- [ ] **Step 2: Full gate**

Run: `just verify` Expected: green.

- [ ] **Step 3: Hand off**

Run the `sdd-gardening` skill for the four `docs/wip/2026-09-13-*` files (two
specs, two plans) before opening the PR, per `sdd-working-memory-lifecycle`.
Then open the PR against `main`. Its description carries the maintainer
checklist — none of these are done by the agent:

```markdown
## Maintainer checklist (before the first editor-v tag)
- [ ] Add repository secrets `VSCE_PAT` and `OVSX_PAT` (publisher `driftsys` on both registries already exists for markspec-ide).
- [ ] Create the `marketplace` GitHub Environment with required reviewers.
- [ ] Bump `editors/vscode/package.json` `version`, commit, tag `editor-v<version>`, push the tag.
- [ ] Approve the two `marketplace` deployments when the release is green.
- [ ] Verify one host end to end: `curl -fsSL .../install.sh | bash`, then `ridl --version` prints the tag, and `claude mcp add ridl -- ridl mcp` lists `ridl_check`.
```
