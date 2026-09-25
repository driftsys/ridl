// The RIDL VS Code extension entry point (docs/ROADMAP.md epics E1.17,
// E2.10b, E6.15).
//
// `activate` registers the "Install ridl to PATH" command and the MCP
// server definition provider unconditionally — both are cheap and spawn no
// process. It starts the LSP client (over stdio against the `ridl` binary,
// run as `ridl lsp`, crates/ridl) only once a `typl`, `ridl` or `rsdl`
// document is open: one already open at activation, or the first one opened
// later. One server serves the three languages: the compiler selects the
// profile from the file extension, so the `.typl`, `.ridl` and `.rsdl` files
// of one workspace are checked together. The same binary also runs as
// `ridl mcp` for the Model Context Protocol server. binaryResolution.ts picks
// the binary in three tiers: the `ridl.serverPath` setting, the binary bundled
// in the extension, then `ridl` resolved from PATH.

import { execFile } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { promisify } from "node:util";
import * as vscode from "vscode";
import {
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  TransportKind,
} from "vscode-languageclient/node";
import {
  clientDocumentSelector,
  isLegacyServerName,
  resolveLspCommand,
  resolveMcpDefinition,
  shouldStartClientForLanguage,
} from "./binaryResolution";
import { ClientLifecycle } from "./clientLifecycle";
import { copyIsStale, isDirOnPath, parseVersionOutput, pathHint, performCopy, planInstall } from "./installToPath";

const MCP_PROVIDER_ID = "ridl";

const execFileAsync = promisify(execFile);

let lifecycle: ClientLifecycle<LanguageClient> | undefined;

// Whether the stale-copy offer has run this activation. It runs at most
// once: when a failed restart dropped the client, or the server stopped, the
// next document open starts a new client, and that start must not offer
// again.
let offeredStaleCopyCheck = false;

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  // Shared by every client the lifecycle creates: a client whose start fails
  // never disposes the output channel it created itself, and
  // vscode-languageclient does not dispose a channel it was given, so one
  // channel outliving every retry avoids leaking a channel per retry. It
  // must be a LogOutputChannel because the library reads `.logLevel` from
  // the supplied channel.
  const outputChannel = vscode.window.createOutputChannel("RIDL Language Server", { log: true });
  context.subscriptions.push(outputChannel);
  lifecycle = new ClientLifecycle(() => createClient(context, outputChannel));
  const configured = configuredServerPath();
  if (configured && isLegacyServerName(configured)) {
    void vscode.window.showWarningMessage(
      "ridl.serverPath names the old ridl-lsp binary. The setting now points at the ridl binary, which the extension runs as `ridl lsp`.",
    );
  }
  registerMcpProvider(context);
  context.subscriptions.push(
    vscode.commands.registerCommand("ridl.installToPath", () => installToPath(context)),
  );
  context.subscriptions.push(
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      // No gate on `lifecycle.client` here: after a failed start the client
      // is undefined, and correcting the setting must still restart. The
      // gate on "a document was opened" lives inside `lifecycle.restart`.
      if (event.affectsConfiguration("ridl.serverPath")) {
        await lifecycle?.restart();
      }
    }),
  );
  context.subscriptions.push(
    vscode.workspace.onDidOpenTextDocument((document) => {
      if (shouldStartClientForLanguage(document.languageId)) {
        void startClientIfNeeded(context);
      }
    }),
  );
  if (vscode.workspace.textDocuments.some((document) => shouldStartClientForLanguage(document.languageId))) {
    await startClientIfNeeded(context);
  }
}

export async function deactivate(): Promise<void> {
  await lifecycle?.stop();
}

/**
 * Starts the language client the first time a `typl`, `ridl` or `rsdl`
 * document is open; a no-op while the client is running. Retried on every
 * later document open after a failed start, since `lifecycle.startIfNeeded`
 * drops the client on failure.
 */
async function startClientIfNeeded(context: vscode.ExtensionContext): Promise<void> {
  if (await lifecycle!.startIfNeeded()) {
    if (!offeredStaleCopyCheck) {
      offeredStaleCopyCheck = true;
      void offerRefreshOfStaleCopy(context);
    }
  }
}

/** The `ridl.serverPath` setting, trimmed, or undefined when blank. */
function configuredServerPath(): string | undefined {
  const value = vscode.workspace.getConfiguration("ridl").get<string>("serverPath");
  return value && value.trim().length > 0 ? value.trim() : undefined;
}

/** Builds the language client: stdio transport to `ridl lsp`, scoped to `.typl`, `.ridl` and `.rsdl` files. */
function createClient(context: vscode.ExtensionContext, outputChannel: vscode.LogOutputChannel): LanguageClient {
  const { command, args } = resolveLspCommand({
    configuredPath: configuredServerPath(),
    extensionPath: context.extensionPath,
    platform: process.platform,
    exists: (file) => fs.existsSync(file),
  });
  const serverOptions: ServerOptions = {
    command,
    args,
    transport: TransportKind.stdio,
  };

  const clientOptions: LanguageClientOptions = {
    documentSelector: clientDocumentSelector(),
    outputChannel,
  };

  // The client id "ridl" ties this client to the `ridl.trace.server` setting:
  // vscode-languageclient reads the trace level from `<id>.trace.server`.
  return new LanguageClient("ridl", "RIDL Language Server", serverOptions, clientOptions);
}

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

/** Runs `binary --version` and parses the version out of its output; undefined when the binary cannot be run. */
async function versionOf(binary: string): Promise<string | undefined> {
  try {
    const { stdout } = await execFileAsync(binary, ["--version"], { timeout: 5000 });
    return parseVersionOutput(stdout);
  } catch {
    return undefined;
  }
}

/** The "RIDL: Install ridl to PATH" command: copies the bundled binary to a directory on PATH. */
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

/** When the installed copy at `plan.target` is older than the bundled binary, offer to re-copy. */
async function offerRefreshOfStaleCopy(context: vscode.ExtensionContext): Promise<void> {
  const plan = planInstall({ platform: process.platform, homedir: os.homedir(), extensionPath: context.extensionPath });
  if (!fs.existsSync(plan.source) || !fs.existsSync(plan.target)) return;
  const [bundled, installed] = await Promise.all([versionOf(plan.source), versionOf(plan.target)]);
  if (!copyIsStale(bundled, installed)) return;
  const pick = await vscode.window.showInformationMessage(
    `${plan.target} is ${installed}; this extension bundles ${bundled}.`,
    "Update the copy",
  );
  if (pick === "Update the copy") {
    await installToPath(context);
  }
}
