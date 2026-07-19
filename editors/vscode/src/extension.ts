// The RIDL VS Code extension entry point (docs/ROADMAP.md epic E1.17).
//
// Activates on the `typl` language and starts an LSP client over stdio
// against the `ridl-lsp` binary (crates/ridl-lsp). The binary path comes
// from the `ridl.serverPath` setting, falling back to `ridl-lsp` resolved
// from PATH.

import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";

let client: LanguageClient | undefined;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  client = createClient();
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

/** Builds the language client: stdio transport to `ridl-lsp`, scoped to `.typl` files. */
function createClient(): LanguageClient {
  const serverOptions: ServerOptions = {
    command: resolveServerPath(),
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: [{ scheme: "file", language: "typl" }],
  };

  // The client id "ridl" ties this client to the `ridl.trace.server` setting:
  // vscode-languageclient reads the trace level from `<id>.trace.server`.
  return new LanguageClient("ridl", "RIDL Language Server", serverOptions, clientOptions);
}

/** The `ridl.serverPath` setting, or `ridl-lsp` resolved from PATH when unset. */
function resolveServerPath(): string {
  const configured = vscode.workspace.getConfiguration("ridl").get<string>("serverPath");
  return configured && configured.trim().length > 0 ? configured : "ridl-lsp";
}

/** Restarts the client after `ridl.serverPath` changes, so the new binary path takes effect. */
async function restartClient(): Promise<void> {
  if (client) {
    await client.stop();
  }
  client = createClient();
  await client.start();
}
