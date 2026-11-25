// Starts one directly selected language server per discovered RIDL root.
import { execFile } from "node:child_process";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { promisify } from "node:util";
import * as vscode from "vscode";
import {
  InitializeParams,
  LanguageClient,
  LanguageClientOptions,
  ServerOptions,
  State,
  TransportKind,
} from "vscode-languageclient/node";
import {
  clientDocumentSelector,
  isLegacyServerName,
  resolveBinary,
  resolveMcpDefinition,
  shouldStartClientForLanguage,
} from "./binaryResolution";
import { ClientPhase, phaseOf } from "./clientLifecycle";
import { copyIsStale, isDirOnPath, parseVersionOutput, pathHint, performCopy, planInstall } from "./installToPath";
import { runOnce } from "./runOnce";
import { BinaryRunner, Selection, ToolchainSelector } from "./toolchainSelection";
import {
  aliasRegistrations,
  canonicalFile,
  routingMiddleware,
  selectFirstWorkspaceFolder,
  WorkspaceClients,
} from "./workspaceClients";

const execFileAsync = promisify(execFile);

/** execFile's callback retains stdout for ordinary nonzero exits, including which exit 1. */
const runner: BinaryRunner = {
  run(command, args, cwd) {
    return new Promise((resolve, reject) => {
      execFile(command, [...args], {
        cwd,
        timeout: args[1] === "install" ? 25 * 60_000 : 15_000,
        maxBuffer: 1024 * 1024,
      }, (error, stdout, stderr) => {
        if (error && (typeof error.code !== "number" || error.killed || error.signal)) {
          reject(error);
          return;
        }
        resolve({ exitCode: error ? error.code as number : 0, stdout, stderr });
      });
    });
  },
};

function libraryStateNumber(state: State): number {
  switch (state) {
    case State.Starting:
    case State.Running:
    case State.StartFailed:
    case State.Stopped:
      return state;
    default: {
      const exhaustive: never = state;
      throw new Error(`unhandled vscode-languageclient state: ${String(exhaustive)}`);
    }
  }
}

class RidlLanguageClient extends LanguageClient {
  readonly opened = new Set<string>();
  manifestless = false;
  refreshAliases: (() => void) | undefined;
  private aliasSignature = "";
  private aliasRegistrations: { method: string; id: string }[] = [];
  private aliasSequence = 0;

  syncAliases(roots: readonly string[]): void {
    const signature = JSON.stringify(roots);
    if (this.phase() !== "running" || this.aliasSignature === signature) return;
    const previous = this.aliasRegistrations;
    this.aliasRegistrations = [];
    const selector = roots.flatMap((root) => clientDocumentSelector(root));
    if (selector.length > 0 && this.initializeResult) {
      for (const registration of aliasRegistrations(this.initializeResult.capabilities, selector)) {
        // All methods here implement the public document registration interface.
        const feature = this.getFeature(registration.method as "textDocument/hover");
        const id = `ridl-alias-${++this.aliasSequence}`;
        feature.register({ id, registerOptions: registration.options });
        this.aliasRegistrations.push({ method: registration.method, id });
      }
    }
    // Replacement registrations must cover documents before old ones are removed.
    for (const { method, id } of previous) this.getFeature(method as "textDocument/hover").unregister(id);
    this.aliasSignature = signature;
  }
  start(): Promise<void> {
    // The library can restart this same client after a server crash.
    if (this.phase() === "stopped" || this.phase() === "startFailed") {
      this.opened.clear();
      this.aliasSignature = "";
      this.aliasRegistrations = [];
    }
    return super.start().then(() => {
      this.refreshAliases?.();
    });
  }
  phase(): ClientPhase {
    return phaseOf(libraryStateNumber(this.state));
  }
  protected fillInitializeParams(params: InitializeParams): void {
    super.fillInitializeParams(params);
    if (this.manifestless) {
      params.rootUri = null;
      params.rootPath = null;
      params.workspaceFolders = null;
    }
  }
}
let clients: WorkspaceClients<RidlLanguageClient> | undefined;
let disposeActivation: (() => void) | undefined;

function isRidlDocument(document: vscode.TextDocument): boolean {
  return document.uri.scheme === "file" &&
    (shouldStartClientForLanguage(document.languageId) || path.basename(document.uri.fsPath) === "ridl.toml");
}

export async function activate(context: vscode.ExtensionContext): Promise<void> {
  const output = vscode.window.createOutputChannel("RIDL Language Server", { log: true });
  context.subscriptions.push(output);
  const active = new Map<RidlLanguageClient, Selection>();
  const offerStaleCopyOnce = runOnce(() => void offerRefreshOfStaleCopy(context));
  const mcpChanged = new vscode.EventEmitter<void>();
  context.subscriptions.push(mcpChanged);
  const approved = new Set<string>(), offered = new Set<string>();
  let mcpSelection: Selection | undefined, mcpGeneration = 0;
  let disposed = false;
  disposeActivation = () => {
    disposed = true;
    ++mcpGeneration;
  };
  const resolver = (directory?: string) =>
    resolveBinary({
      configuredPath: vscode.workspace.getConfiguration("ridl", directory ? vscode.Uri.file(directory) : undefined).get<
        string
      >("serverPath"),
      extensionPath: context.extensionPath,
      platform: process.platform,
      exists: fs.existsSync,
    });
  const selector = new ToolchainSelector(runner, {
    progress: async (title, work) =>
      vscode.window.withProgress({ location: vscode.ProgressLocation.Notification, title }, work),
    // Return the fallback immediately while the nonmodal Install button is pending.
    // An accepted release is selected again through the same validated CLI path.
    async offerInstall(pin) {
      if (approved.has(pin)) return true;
      if (!offered.has(pin)) {
        offered.add(pin);
        void Promise.resolve(
          vscode.window.showInformationMessage(`RIDL ${pin} is required by this workspace.`, "Install"),
        ).then(async (button) => {
          if (button === "Install" && !disposed) {
            approved.add(pin);
            await manager.refresh("");
            await refreshMcp();
          }
        }).catch(logError);
      }
      return false;
    },
    errorOnce: (_key, message) => {
      void vscode.window.showErrorMessage(message);
    },
    log: (message) => output.warn(message),
  });
  const select = async (directory: string): Promise<Selection> => {
    const selected = await selector.select({
      resolver: resolver(directory),
      directory,
      autoInstall: vscode.workspace.getConfiguration("ridl").get<boolean>("toolchain.autoInstall", true),
    });
    if (!selected.key.startsWith("folder:")) return selected;
    const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(directory))?.uri.fsPath ?? directory;
    return { ...selected, key: `folder:${path.resolve(folder)}`, root: path.resolve(folder) };
  };
  function logError(error: unknown): void {
    output.error(String(error));
  }
  async function refreshMcp(): Promise<void> {
    const generation = ++mcpGeneration;
    const selected = await selectFirstWorkspaceFolder(
      vscode.workspace.workspaceFolders?.map((f) => f.uri.fsPath) ?? [],
      select,
    );
    if (disposed || generation !== mcpGeneration) return;
    if (selected?.command !== mcpSelection?.command || selected?.key !== mcpSelection?.key) {
      mcpSelection = selected;
      mcpChanged.fire();
    }
  }
  const manager = new WorkspaceClients(select, (selection) => {
    const client = createClient(
      selection,
      output,
      (file) => manager.owns(selection.key, file),
      (file) => manager.hostPath(selection.key, file),
    );
    client.refreshAliases = () => client.syncAliases(manager.selectorRoots(selection.key).slice(1));
    active.set(client, selection);
    return client;
  });
  clients = manager;
  manager.onChange = async () => {
    for (const [client, selection] of active) {
      if (!manager.hasClient(client)) {
        active.delete(client);
        continue;
      }
      if (client.phase() !== "running") continue;
      client.refreshAliases?.();
      // Remove diagnostics and open overlays whose ownership has moved to a nested root.
      client.diagnostics?.forEach((uri) => {
        if (!manager.owns(selection.key, uri.fsPath)) client.diagnostics?.delete(uri);
      });
      for (const document of vscode.workspace.textDocuments) {
        if (!isRidlDocument(document)) continue;
        const owned = manager.owns(selection.key, document.uri.fsPath);
        const uri = canonicalUri(document.uri);
        if (!owned && client.opened.has(uri)) {
          await client.sendNotification("textDocument/didClose", { textDocument: { uri } });
        } else if (owned && !client.opened.has(uri)) {
          await client.sendNotification("textDocument/didOpen", {
            textDocument: { uri, languageId: document.languageId, version: document.version, text: document.getText() },
          });
        }
      }
    }
    if ([...active.keys()].some((client) => client.phase() === "running")) offerStaleCopyOnce();
  };
  async function open(document: vscode.TextDocument): Promise<void> {
    if (isRidlDocument(document)) await manager.open(path.dirname(document.uri.fsPath));
  }
  // Forward before reselection: deletion/change must also reach the old server
  // when selection keeps its binary or removes its root.
  async function watched(uri: vscode.Uri, type: number): Promise<void> {
    for (const client of active.keys()) {
      if (client.phase() === "starting") await client.start().catch(logError);
      if (client.phase() === "running") {
        await client.sendNotification("workspace/didChangeWatchedFiles", {
          changes: [{ uri: canonicalUri(uri), type }],
        });
      }
    }
    await manager.refresh(uri.fsPath);
    await refreshMcp();
  }
  const watcher = vscode.workspace.createFileSystemWatcher("**/ridl.toml");
  context.subscriptions.push(
    watcher,
    watcher.onDidCreate(async (uri) => {
      try {
        await watched(uri, 1);
      } catch (error) {
        logError(error);
      }
    }),
    watcher.onDidChange(async (uri) => {
      try {
        await watched(uri, 2);
      } catch (error) {
        logError(error);
      }
    }),
    watcher.onDidDelete(async (uri) => {
      try {
        await watched(uri, 3);
      } catch (error) {
        logError(error);
      }
    }),
    vscode.workspace.onDidOpenTextDocument(async (document) => {
      try {
        await open(document);
      } catch (error) {
        logError(error);
      }
    }),
    vscode.workspace.onDidChangeConfiguration(async (event) => {
      if (event.affectsConfiguration("ridl.serverPath") || event.affectsConfiguration("ridl.toolchain.autoInstall")) {
        try {
          await manager.refresh("");
          await refreshMcp();
        } catch (error) {
          logError(error);
        }
      }
    }),
    vscode.workspace.onDidChangeWorkspaceFolders(async (event) => {
      try {
        for (const folder of event.removed) await manager.removeFolder(folder.uri.fsPath);
        await manager.refresh("");
        for (const document of vscode.workspace.textDocuments) {
          if (vscode.workspace.getWorkspaceFolder(document.uri)) await open(document);
        }
        await refreshMcp();
      } catch (error) {
        logError(error);
      }
    }),
    vscode.commands.registerCommand("ridl.installToPath", () => installToPath(context)),
    { dispose: disposeActivation },
    vscode.lm.registerMcpServerDefinitionProvider("ridl", {
      onDidChangeMcpServerDefinitions: mcpChanged.event,
      provideMcpServerDefinitions: async () => {
        await refreshMcp();
        const folder = vscode.workspace.workspaceFolders?.[0]?.uri.fsPath;
        const resolved = resolveMcpDefinition({
          configuredPath: resolver(folder).source === "setting" ? resolver(folder).command : undefined,
          extensionPath: context.extensionPath,
          platform: process.platform,
          exists: fs.existsSync,
          extensionVersion: String(context.extension.packageJSON.version),
          workspaceFolder: folder,
          selectedCommand: mcpSelection?.command,
        });
        const definition = new vscode.McpStdioServerDefinition(
          resolved.label,
          resolved.command,
          resolved.args,
          {},
          resolved.version,
        );
        if (resolved.cwd) definition.cwd = vscode.Uri.file(resolved.cwd);
        return [definition];
      },
    }),
  );
  for (const folder of vscode.workspace.workspaceFolders ?? []) {
    const configured = resolver(folder.uri.fsPath);
    if (configured.source === "setting" && isLegacyServerName(configured.command)) {
      void vscode.window.showWarningMessage(
        "ridl.serverPath names the old ridl-lsp binary. The setting now points at the ridl binary, which the extension runs as `ridl lsp`.",
      );
      break;
    }
  }
  await Promise.all(vscode.workspace.textDocuments.map(open));
}

export async function deactivate(): Promise<void> {
  disposeActivation?.();
  await clients?.stop();
}

function canonicalUri(uri: vscode.Uri): string {
  return uri.scheme === "file" ? vscode.Uri.file(canonicalFile(uri.fsPath)).toString() : uri.toString();
}

function createClient(
  selection: Selection,
  outputChannel: vscode.LogOutputChannel,
  owns: (file: string) => boolean,
  hostPath: (file: string) => string,
): RidlLanguageClient {
  const serverOptions: ServerOptions = { command: selection.command, args: ["lsp"], transport: TransportKind.stdio };
  const documentSelector = clientDocumentSelector(selection.root === null ? undefined : canonicalFile(selection.root));
  if (selection.root === null) documentSelector.push({ scheme: "file", language: "*", pattern: "**/ridl.toml" });
  const middleware = routingMiddleware(owns);
  const clientOptions: LanguageClientOptions = {
    documentSelector,
    outputChannel,
    middleware,
    uriConverters: {
      code2Protocol: canonicalUri,
      protocol2Code: (value) => {
        const uri = vscode.Uri.parse(value);
        if (uri.scheme !== "file") return uri;
        const file = canonicalFile(uri.fsPath);
        return vscode.workspace.textDocuments.find((document) =>
          document.uri.scheme === "file" && canonicalFile(document.uri.fsPath) === file
        )?.uri ??
          vscode.Uri.file(hostPath(file));
      },
    },
    ...(selection.root === null
      ? {}
      : { workspaceFolder: { uri: vscode.Uri.file(selection.root), name: path.basename(selection.root), index: 0 } }),
  };
  const client = new RidlLanguageClient("ridl", "RIDL Language Server", serverOptions, clientOptions);
  client.manifestless = selection.root === null;
  middleware.sendNotification = async (type, next, params) => {
    const method = typeof type === "string" ? type : type.method;
    const document = (params as { textDocument?: { uri?: string } } | undefined)?.textDocument;
    if (document?.uri) {
      const uri = vscode.Uri.parse(document.uri);
      if (method !== "textDocument/didClose" && (uri.scheme !== "file" || !owns(uri.fsPath))) return;
      if (method === "textDocument/didOpen") {
        if (client.opened.has(document.uri)) return;
        client.opened.add(document.uri);
      } else if (method === "textDocument/didClose") {
        if (!client.opened.delete(document.uri)) return;
      }
    }
    await next(type, params);
  };
  return client;
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
