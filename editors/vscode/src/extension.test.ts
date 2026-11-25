import { strict as assert } from "node:assert";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { test } from "node:test";
import Module = require("node:module");
import { minimatch } from "minimatch";

// Replace only the host and process boundaries. Selection, routing and lifecycle
// remain the production implementations used by activate().
const listeners = new Map<string, ((...args: any[]) => any)[]>();
const register = (event: string) => (fn: (...args: any[]) => any) => {
  listeners.set(event, [...listeners.get(event) ?? [], fn]);
  return { dispose() {} };
};
async function fire(event: string, value: any) {
  for (const fn of listeners.get(event) ?? []) await fn(value);
}
const abs = (s: string) => path.resolve(s);
const uri = (s: string) => ({
  scheme: "file",
  fsPath: abs(s),
  toString() {
    return pathToFileURL(this.fsPath).href;
  },
});
const document = (s: string, languageId = "ridl") => ({
  uri: uri(s),
  languageId,
  version: 1,
  getText: () => "[package]\nname = 'p'\n",
});
const clients: FakeClient[] = [];
class FakeClient {
  initializeResult = {
    capabilities: {
      textDocumentSync: { openClose: true, change: 2 },
      hoverProvider: true,
      completionProvider: { triggerCharacters: ["."] },
      definitionProvider: true,
      referencesProvider: true,
      inlayHintProvider: true,
      documentFormattingProvider: true,
      documentSymbolProvider: true,
      renameProvider: { prepareProvider: true },
      codeActionProvider: { codeActionKinds: ["refactor.rewrite"] },
    },
  };
  initialized: any;
  features = new Map<string, { registrations: any[]; register(data: any): void; unregister(id: string): void }>();
  fillInitializeParams(params: any) {}
  getFeature(method: string) {
    let feature = this.features.get(method);
    if (!feature) {
      feature = {
        registrations: [],
        register(data) {
          this.registrations.push(data);
        },
        unregister(id) {
          this.registrations = this.registrations.filter((r) => r.id !== id);
        },
      };
      this.features.set(method, feature);
    }
    return feature;
  }
  state = 1;
  stops = 0;
  messages: any[] = [];
  diagnostics = { forEach() {}, delete() {} };
  constructor(readonly id: string, readonly name: string, readonly serverOptions: any, readonly clientOptions: any) {
    clients.push(this);
  }
  async start() {
    this.initialized = {
      rootUri: this.clientOptions.workspaceFolder?.uri.toString(),
      workspaceFolders: [this.clientOptions.workspaceFolder],
    };
    this.fillInitializeParams(this.initialized);
    this.state = 2;
  }
  async stop() {
    this.state = 1;
    this.stops++;
  }
  async sendNotification(method: any, params: any) {
    const next = async (type: any, value: any) => {
      this.messages.push([typeof type === "string" ? type : type.method, value]);
    };
    await (this.clientOptions.middleware?.sendNotification?.(method, next, params) ?? next(method, params));
  }
}
let provider: any, mcpChanges = 0;
let command = "/selected/a", whichCalls = 0, installCalls = 0;
let autoInstall = true, missingRelease = false, nestedRoot = true;
let acceptInstall: ((button: string | undefined) => void) | undefined;
const folderServerPaths = new Map<string, string>();
const installPrompts: string[][] = [];
const configurationReads: [string, string | undefined][] = [];
const folders = [{ uri: uri("/work"), name: "work", index: 0 }];
const documents: ReturnType<typeof document>[] = [];
const watcher = {
  onDidCreate: register("create"),
  onDidChange: register("change"),
  onDidDelete: register("delete"),
  dispose() {},
};
const host: any = {
  Uri: { file: uri, parse: (value: string) => uri(fileURLToPath(value)) },
  EventEmitter: class {
    event = () => ({ dispose() {} });
    fire() {
      mcpChanges++;
    }
    dispose() {}
  },
  ProgressLocation: { Notification: 15 },
  window: {
    createOutputChannel: () => ({ warn() {}, error() {}, info() {}, dispose() {} }),
    showWarningMessage: async () => undefined,
    showErrorMessage: async () => undefined,
    showInformationMessage: async (message: string, ...buttons: string[]) => {
      if (message.includes("required by this workspace")) installPrompts.push(buttons);
      return message.includes("required by this workspace")
        ? new Promise<string | undefined>((resolve) => {
          acceptInstall = resolve;
        })
        : undefined;
    },
    withProgress: async (_: any, work: any) => work(),
  },
  commands: { registerCommand: () => ({ dispose() {} }) },
  workspace: {
    textDocuments: documents,
    workspaceFolders: folders,
    getWorkspaceFolder: (u: any) => folders.find((f) => u.fsPath.startsWith(f.uri.fsPath)),
    getConfiguration: (_namespace: string, resource: any) => ({
      get: (name: string, fallback: any) => {
        configurationReads.push([name, resource?.fsPath]);
        if (name === "serverPath") {
          const scope = require("../package.json").contributes.configuration.properties["ridl.serverPath"].scope;
          const folder = resource && host.workspace.getWorkspaceFolder(resource);
          return scope === "resource" && folder ? folderServerPaths.get(folder.uri.fsPath) ?? "" : "";
        }
        return name === "toolchain.autoInstall" ? autoInstall : fallback;
      },
    }),
    onDidChangeConfiguration: register("config"),
    onDidOpenTextDocument: register("open"),
    onDidChangeWorkspaceFolders: register("folders"),
    createFileSystemWatcher: (glob: string) => {
      assert.equal(glob, "**/ridl.toml");
      return watcher;
    },
  },
  lm: {
    registerMcpServerDefinitionProvider: (_: any, p: any) => {
      provider = p;
      return { dispose() {} };
    },
  },
  McpStdioServerDefinition: class {
    constructor(
      readonly label: string,
      readonly command: string,
      readonly args: string[],
      readonly env: any,
      readonly version: string,
    ) {}
  },
};
const loader = Module as any;
const original = loader._load;
loader._load = function (id: string, ...rest: any[]) {
  if (id === "vscode") return host;
  if (id === "vscode-languageclient/node") {
    return {
      LanguageClient: FakeClient,
      State: { Starting: 3, Running: 2, StartFailed: 4, Stopped: 1 },
      TransportKind: { stdio: 0 },
    };
  }
  if (id === "node:child_process") {
    return {
      execFile: (binary: string, args: string[], options: any, done: any) => {
        if (args[1] === "install") {
          installCalls++;
          missingRelease = false;
          done(null, "ignored", "");
          return;
        }
        whichCalls++;
        const d = args[3];
        if (d.startsWith(abs("/loose"))) {
          done(
            null,
            JSON.stringify({
              schema: 1,
              manifest: null,
              running: "0.23.0",
              pin: null,
              status: "unpinned",
              path: "/resolver/ridl",
              source: "running",
              diagnostic: null,
            }),
            "",
          );
          return;
        }
        const root = nestedRoot && d.startsWith(abs("/work/nested"))
          ? abs("/work/nested")
          : fs.existsSync(path.join(d, "ridl.toml"))
          ? fs.realpathSync.native(d)
          : abs("/work");
        if (missingRelease) {
          done(
            Object.assign(new Error("which exit 1"), { code: 1 }),
            JSON.stringify({
              schema: 1,
              manifest: path.join(root, "ridl.toml"),
              running: "0.24.0",
              pin: "0.23.0",
              status: "missing",
              path: null,
              source: null,
              diagnostic: null,
            }),
            "",
          );
          return;
        }
        done(
          null,
          JSON.stringify({
            schema: 1,
            manifest: path.join(root, "ridl.toml"),
            running: "0.23.0",
            pin: "0.23.0",
            status: "compatible",
            path: root.endsWith("nested") ? "/selected/nested" : command,
            source: "installed",
            diagnostic: null,
          }),
          "",
        );
      },
    };
  }
  return original.call(this, id, ...rest);
};
const extension = require("./extension") as typeof import("./extension");
loader._load = original;

test("activation_wires_toml_roots_watchers_mcp_and_deactivation", async () => {
  documents.push(document("/work/ridl.toml", "plaintext"));
  const context: any = {
    extensionPath: "/nonexistent-extension",
    extension: { packageJSON: { version: "0.23.0" } },
    subscriptions: [],
  };
  await extension.activate(context);
  assert.equal(clients.length, 1, "an opened root TOML starts the pin-action client");
  assert.ok(
    clients[0].getFeature("textDocument/codeAction").registrations.at(-1).registerOptions.documentSelector.some((
      entry: any,
    ) => entry.language === "*" && entry.pattern?.endsWith("/ridl.toml")),
    "manifest providers register in plaintext as well as TOML mode",
  );
  assert.equal(clients[0].serverOptions.command, "/selected/a");
  assert.deepEqual(clients[0].serverOptions.args, ["lsp"]);
  assert.equal(clients[0].clientOptions.workspaceFolder.uri.fsPath, abs("/work"));
  assert.equal(clients[0].messages.filter(([method]) => method === "textDocument/didOpen").length, 1);
  const ignored = document("/work/config.toml", "toml");
  documents.push(ignored);
  await fire("open", ignored);
  assert.equal(clients.length, 1);
  const nested = document("/work/nested/a.ridl");
  documents.push(nested);
  await fire("open", nested);
  assert.equal(clients.length, 2);
  assert.equal(clients[1].clientOptions.workspaceFolder.uri.fsPath, abs("/work/nested"));
  const definitions = await provider.provideMcpServerDefinitions();
  assert.equal(definitions[0].command, "/selected/a");
  assert.deepEqual(definitions[0].args, ["mcp"]);
  command = "/selected/new";
  const beforeMcpChanges = mcpChanges;
  await fire("change", uri("/work/ridl.toml"));
  assert.equal(clients[0].stops, 1);
  assert.equal(clients[1].stops, 0);
  assert.ok(
    clients[0].messages.some(([method, value]) =>
      method === "workspace/didChangeWatchedFiles" && value.changes[0].type === 2
    ),
  );
  assert.ok(mcpChanges > beforeMcpChanges);
  assert.equal((await provider.provideMcpServerDefinitions())[0].command, "/selected/new");
  assert.ok(whichCalls >= 4);
  assert.equal(installCalls, 0);
  const current = clients.at(-1)!;
  await fire("create", uri("/work/ridl.toml"));
  await fire("delete", uri("/work/ridl.toml"));
  assert.deepEqual(
    current.messages.filter(([method, value]) => method === "workspace/didChangeWatchedFiles").map(([, value]) =>
      value.changes[0].type
    ),
    [1, 3],
  );
  await extension.deactivate();
  assert.ok(clients.every((client) => client.stops === 1));
});

test("library_restart_resends_open_buffers_to_the_new_server", async () => {
  command = "/selected/new";
  const offset = clients.length;
  await extension.activate(
    {
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any,
  );
  const client = clients.slice(offset).find((c) => c.serverOptions.command === "/selected/new")!;
  const doc = document("/work/a.ridl");
  await client.sendNotification("textDocument/didOpen", { textDocument: { uri: doc.uri.toString(), text: "unsaved" } });
  const count = client.messages.length;
  client.state = 1;
  await client.start();
  await client.sendNotification("textDocument/didOpen", { textDocument: { uri: doc.uri.toString(), text: "unsaved" } });
  assert.equal(client.messages.length, count + 1);
  await extension.deactivate();
});

async function tickUntil(predicate: () => boolean): Promise<void> {
  for (let i = 0; i < 50 && !predicate(); i++) await new Promise((resolve) => setImmediate(resolve));
  assert.ok(predicate(), "asynchronous host effect completed");
}
function resetHost(): void {
  listeners.clear();
  clients.splice(0);
  documents.splice(0, documents.length, document("/work/ridl.toml", "plaintext"));
  autoInstall = false;
  missingRelease = true;
  nestedRoot = true;
  acceptInstall = undefined;
  installPrompts.splice(0);
  folderServerPaths.clear();
  folders.splice(0, folders.length, { uri: uri("/work"), name: "work", index: 0 });
}
test("application_policy_starts_fallback_before_install_button_is_answered", async () => {
  resetHost();
  const before = installCalls;
  await extension.activate(
    {
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any,
  );
  assert.equal(clients.length, 1);
  assert.equal(clients[0].serverOptions.command, "ridl");
  assert.equal(installCalls, before);
  assert.ok(acceptInstall);
  assert.ok(
    configurationReads.filter(([name]) => name === "toolchain.autoInstall").every(([, resource]) =>
      resource === undefined
    ),
  );
  acceptInstall!("Install");
  await tickUntil(() => clients.length === 2);
  assert.equal(installCalls, before + 1);
  assert.equal(clients[1].serverOptions.command, "/selected/new");
  await extension.deactivate();
});
test("deactivation_cancels_pending_install_prompt", async () => {
  resetHost();
  const before = installCalls;
  await extension.activate(
    {
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any,
  );
  assert.ok(acceptInstall);
  await extension.deactivate();
  acceptInstall!("Install");
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(installCalls, before);
  assert.equal(clients.length, 1);
});

test("symlink_aliases_keep_scoped_providers_canonical_buffers_and_one_client", async () => {
  resetHost();
  autoInstall = true;
  missingRelease = false;
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "ridl-host-alias-"));
  try {
    const real = path.join(base, "real"), first = path.join(base, "first"), second = path.join(base, "second");
    fs.mkdirSync(real);
    fs.writeFileSync(path.join(real, "ridl.toml"), "[package]\nname='p'\n");
    fs.symlinkSync(real, first, "junction");
    fs.symlinkSync(real, second, "junction");
    const manifest = document(path.join(first, "ridl.toml"), "plaintext");
    documents.splice(0, documents.length, manifest);
    await extension.activate(
      {
        extensionPath: "/nonexistent-extension",
        extension: { packageJSON: { version: "0.23.0" } },
        subscriptions: [],
      } as any,
    );
    const client = clients[0],
      canonical = process.platform === "win32" ? path.resolve(real) : fs.realpathSync.native(real);
    const converters = client.clientOptions.uriConverters;
    assert.ok(converters, "host and server URIs use one filesystem identity");
    const canonicalUri = pathToFileURL(path.join(canonical, "ridl.toml")).href;
    assert.equal(converters.code2Protocol(manifest.uri), canonicalUri);
    assert.equal(converters.protocol2Code(canonicalUri).fsPath, manifest.uri.fsPath);
    assert.equal(client.initialized.rootUri, pathToFileURL(canonical).href);
    assert.ok(
      client.getFeature("textDocument/codeAction").registrations.some((r) =>
        r.registerOptions.documentSelector.some((s: any) =>
          s.language === "*" && s.pattern === first.replace(/\\/g, "/") + "/ridl.toml"
        )
      ),
    );
    const source = document(path.join(second, "new.ridl"));
    documents.push(source);
    await fire("open", source);
    assert.equal(clients.length, 1);
    assert.equal(client.stops, 0);
    assert.ok(
      client.getFeature("textDocument/hover").registrations.at(-1).registerOptions.documentSelector.some((s: any) =>
        s.pattern === second.replace(/\\/g, "/") + "/new.ridl"
      ),
    );
    assert.ok(
      client.messages.some(([method, params]) =>
        method === "textDocument/didOpen" &&
        params.textDocument.uri === pathToFileURL(path.join(canonical, "new.ridl")).href
      ),
    );
    let changes = 0;
    await client.clientOptions.middleware.didChange({ document: manifest }, async () => {
      changes++;
    });
    assert.equal(changes, 1);
    await extension.deactivate();
  } finally {
    fs.rmSync(base, { recursive: true, force: true });
  }
});

test("manifest_watch_event_waits_for_an_automatic_restart", async () => {
  resetHost();
  autoInstall = true;
  missingRelease = false;
  await extension.activate(
    {
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any,
  );
  const client = clients[0];
  client.state = 3;
  await fire("change", uri("/work/ridl.toml"));
  assert.ok(client.messages.some(([method]) => method === "workspace/didChangeWatchedFiles"));
  await extension.deactivate();
});

test("manifestless_client_initialization_does_not_inherit_the_window_root", async () => {
  resetHost();
  autoInstall = true;
  missingRelease = false;
  documents.splice(0, documents.length, document("/loose/new.ridl"));
  await extension.activate(
    {
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any,
  );
  assert.equal(clients[0].initialized.rootUri, null);
  assert.equal(clients[0].initialized.workspaceFolders, null);
  await extension.deactivate();
});

test("parent_registering_after_nested_root_preserves_every_document_provider", async () => {
  resetHost();
  autoInstall = true;
  missingRelease = false;
  const nested = document("/work/nested/a.ridl");
  documents.splice(0, documents.length, nested);
  await extension.activate({
    extensionPath: "/nonexistent-extension",
    extension: { packageJSON: { version: "0.23.0" } },
    subscriptions: [],
  } as any);
  const parentDocument = document("/work/b.ridl");
  documents.push(parentDocument);
  await fire("open", parentDocument);
  assert.deepEqual(clients.map((c) => c.serverOptions.command), ["/selected/nested", command]);
  const methods = [
    "textDocument/formatting",
    "textDocument/hover",
    "textDocument/completion",
    "textDocument/definition",
    "textDocument/references",
    "textDocument/rename",
    "textDocument/codeAction",
    "textDocument/inlayHint",
    "textDocument/documentSymbol",
  ];
  // VS Code gives matching absolute patterns the same score and prefers the
  // most recently registered formatter. Exercise the selectors we register.
  const matches = (selector: any[], doc: ReturnType<typeof document>) =>
    selector.some((s) =>
      s.scheme === doc.uri.scheme && (s.language === "*" || s.language === doc.languageId) &&
      (!s.pattern || minimatch(doc.uri.fsPath.replace(/\\/g, "/"), s.pattern))
    );
  for (const method of methods) {
    const candidates = clients.filter((client) =>
      matches(client.clientOptions.documentSelector, nested) ||
      client.getFeature(method).registrations.some((r) => matches(r.registerOptions.documentSelector, nested))
    );
    assert.deepEqual(candidates.map((c) => c.serverOptions.command), ["/selected/nested"], method);
    if (method === "textDocument/formatting") {
      const chosen = candidates.at(-1)!;
      const edits = [{ newText: "formatted by nested" }];
      const result = await chosen.clientOptions.middleware.sendRequest(
        method,
        { textDocument: { uri: nested.uri.toString() } },
        undefined,
        async () => edits,
      );
      assert.deepEqual(result, edits);
    }
  }
  await extension.deactivate();
});

test("creating_a_nested_root_moves_existing_formatting_registration_to_its_client", async () => {
  resetHost();
  autoInstall = true;
  missingRelease = false;
  nestedRoot = false;
  const source = document("/work/nested/a.ridl");
  documents.splice(0, documents.length, source);
  await extension.activate({
    extensionPath: "/nonexistent-extension",
    extension: { packageJSON: { version: "0.23.0" } },
    subscriptions: [],
  } as any);
  assert.equal(clients[0].serverOptions.command, command);
  nestedRoot = true;
  await fire("create", uri("/work/nested/ridl.toml"));
  const providers = clients.filter((client) =>
    client.state === 2 &&
    client.getFeature("textDocument/formatting").registrations.some((r) =>
      r.registerOptions.documentSelector.some((s: any) => minimatch(source.uri.fsPath.replace(/\\/g, "/"), s.pattern))
    )
  );
  assert.deepEqual(providers.map((client) => client.serverOptions.command), ["/selected/nested"]);
  const edits = [{ newText: "formatted after root creation" }];
  assert.deepEqual(
    await providers[0].clientOptions.middleware.sendRequest(
      "textDocument/formatting",
      { textDocument: { uri: source.uri.toString() } },
      undefined,
      async () => edits,
    ),
    edits,
  );
  await extension.deactivate();
});

test("folder_server_path_is_used_for_each_folders_document", async () => {
  resetHost();
  missingRelease = false;
  folders.push({ uri: uri("/other"), name: "other", index: 1 });
  folderServerPaths.set(abs("/work"), "/custom/work-ridl");
  folderServerPaths.set(abs("/other"), "/custom/other-ridl");
  documents.splice(0, documents.length, document("/work/src/a.ridl"), document("/other/src/b.ridl"));
  const before = whichCalls;
  try {
    await extension.activate({
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any);
    assert.deepEqual(clients.map((client) => client.serverOptions.command), ["/custom/work-ridl", "/custom/other-ridl"]);
    assert.equal(whichCalls, before, "folder overrides bypass release selection");
  } finally {
    await extension.deactivate();
    folderServerPaths.clear();
  }
});

test("activation_auto_install_true_installs_a_missing_release", async () => {
  resetHost();
  autoInstall = true;
  const before = installCalls;
  try {
    await extension.activate({
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any);
    assert.equal(installCalls, before + 1);
    assert.equal(clients[0].serverOptions.command, command);
    assert.deepEqual(installPrompts, [], "automatic installation needs no Install button");
  } finally {
    await extension.deactivate();
  }
});

test("activation_auto_install_false_offers_install_without_running_it", async () => {
  resetHost();
  const before = installCalls;
  try {
    await extension.activate({
      extensionPath: "/nonexistent-extension",
      extension: { packageJSON: { version: "0.23.0" } },
      subscriptions: [],
    } as any);
    assert.equal(installCalls, before);
    assert.deepEqual(installPrompts, [["Install"]]);
    assert.equal(clients[0].serverOptions.command, "ridl");
  } finally {
    await extension.deactivate();
  }
});
