// The RIDL VS Code extension entry point (docs/ROADMAP.md epics E1.17,
// E2.10b).
//
// Activates on the `typl` and `ridl` languages and starts one LSP client over
// stdio against the `ridl` binary, run as `ridl lsp` (crates/ridl). One
// server serves both languages: the compiler selects the profile from the
// file extension, so a `.ridl` file and a `.typl` file of the same package
// are checked together. The same binary also runs as `ridl mcp` for the
// Model Context Protocol server. binaryResolution.ts picks the binary in
// three tiers: the `ridl.serverPath` setting, the binary bundled in the
// extension, then `ridl` resolved from PATH.

import * as fs from "node:fs";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import { isLegacyServerName, LSP_ARGS, resolveBinary } from "./binaryResolution";

let client: LanguageClient | undefined;
let extensionContext: vscode.ExtensionContext | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  extensionContext = context;
  const configured = configuredServerPath();
  if (configured && isLegacyServerName(configured)) {
    void vscode.window.showWarningMessage(
      "ridl.serverPath names the old ridl-lsp binary. The setting now points at the ridl binary, which the extension runs as `ridl lsp`.",
    );
  }
  client = createClient(context);
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (event.affectsConfiguration("ridl.serverPath")) {
        await restartClient();
      }
    }),
  );
  await client.start();
}

export async function deactivate(): Promise<void> {
  if (client) {
    await client.stop();
    client = undefined;
  }
}

/** The `ridl.serverPath` setting, trimmed, or undefined when blank. */
function configuredServerPath(): string | undefined {
  const value = vscode.workspace.getConfiguration("ridl").get<string>("serverPath");
  return value && value.trim().length > 0 ? value.trim() : undefined;
}

/** Builds the language client: stdio transport to `ridl lsp`, scoped to `.typl` and `.ridl` files. */
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

  const clientOptions: LanguageClientOptions = {
    documentSelector: [
      { scheme: "file", language: "typl" },
      { scheme: "file", language: "ridl" },
    ],
  };

  // The client id "ridl" ties this client to the `ridl.trace.server` setting:
  // vscode-languageclient reads the trace level from `<id>.trace.server`.
  return new LanguageClient("ridl", "RIDL Language Server", serverOptions, clientOptions);
}

/** Restarts the client after `ridl.serverPath` changes, so the new binary path takes effect. */
async function restartClient(): Promise<void> {
  if (client) {
    await client.stop();
  }
  client = createClient(extensionContext!);
  await client.start();
}
