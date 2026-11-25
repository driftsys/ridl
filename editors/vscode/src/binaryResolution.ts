// Resolves the bootstrap `ridl` binary used for toolchain discovery. Pure —
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

// The subcommand each role runs the binary as. For the language server,
// vscode-languageclient appends `--stdio` to LSP_ARGS, because extension.ts
// sets `TransportKind.stdio`, so the binary runs as `ridl lsp --stdio`. The
// contract that the `ridl` binary accepts both command lines is verified end
// to end, against the real binary, in crates/ridl/tests/servers.rs.
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
  readonly selectedCommand?: string;
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
  const command = input.selectedCommand ?? resolveBinary(input).command;
  return {
    label: "RIDL",
    command,
    args: [...MCP_ARGS],
    cwd: input.workspaceFolder,
    version: input.extensionVersion,
  };
}

/** The command the language client spawns: the resolved binary, run as `ridl lsp`. */
export function resolveLspCommand(input: ResolveInput): { command: string; args: string[] } {
  const { command } = resolveBinary(input);
  return { command, args: [...LSP_ARGS] };
}

// The language ids the language client serves. The `.typl`, `.ridl` and
// `.rsdl` files of one workspace are checked together by one server (see
// extension.ts).
const CLIENT_LANGUAGE_IDS = ["typl", "ridl", "rsdl"] as const;

/** Whether an open document of this language id is a reason to start the language client. */
export function shouldStartClientForLanguage(languageId: string): boolean {
  return (CLIENT_LANGUAGE_IDS as readonly string[]).includes(languageId);
}

/** Absolute globs escape metacharacters in actual directory names. Nested roots
 * are excluded by routing middleware because VS Code globs cannot express it. */
export function clientDocumentSelector(
  root?: string,
  excludedRoots: readonly string[] = [],
): { scheme: string; language: string; pattern?: string }[] {
  const glob = root?.replace(/\\/g, "/").replace(/[\[\]{}*?!()]/g, "[$&]").replace(/\/$/, "");
  const selectors: { scheme: string; language: string; pattern?: string }[] = CLIENT_LANGUAGE_IDS.map((language) => ({
    scheme: "file",
    language,
    ...(glob === undefined ? {} : { pattern: `${glob}/**/*` }),
  }));
  if (glob !== undefined) selectors.push({ scheme: "file", language: "*", pattern: `${glob}/ridl.toml` });
  return selectors;
}

/** Rust canonicalization on Windows uses verbatim paths; VS Code uses ordinary drive/UNC paths. */
export function normalizeAbsolutePath(file: string, platform: NodeJS.Platform = process.platform): string {
  if (platform === "win32") {
    if (file.startsWith("\\\\?\\UNC\\")) file = "\\\\" + file.slice(8);
    else if (file.startsWith("\\\\?\\")) file = file.slice(4);
    return path.win32.normalize(file);
  }
  return path.normalize(file);
}
