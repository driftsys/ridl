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

// The subcommand each role runs the binary as. The contract that the `ridl`
// binary accepts these subcommands is verified end to end, against the real
// binary, in crates/ridl/tests/servers.rs.
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
