import * as fs from "node:fs";
import * as path from "node:path";
import { fileURLToPath } from "node:url";
import { CancellationToken, LSPErrorCodes, ResponseError } from "vscode-languageserver-protocol";
import type { Middleware } from "vscode-languageclient/node";
import { ClientLifecycle, LifecycleClient } from "./clientLifecycle";
import { clientDocumentSelector, normalizeAbsolutePath } from "./binaryResolution";
import { Selection } from "./toolchainSelection";

function inside(root: string, file: string): boolean {
  const relative = path.relative(root, file);
  return relative === "" || (!relative.startsWith(`..${path.sep}`) && relative !== ".." && !path.isAbsolute(relative));
}
interface Entry<C extends LifecycleClient> {
  generation: number;
  selection: Selection;
  lifecycle: ClientLifecycle<C>;
}

/** Discovery is concurrent; applying selections and changing client ownership is serialized. */
export class WorkspaceClients<C extends LifecycleClient> {
  private readonly directories = new Map<string, Selection>();
  private readonly generations = new Map<string, number>();
  private readonly entries = new Map<string, Entry<C>>();
  private queue: Promise<unknown> = Promise.resolve();
  private stopped = false;
  private sequence = 0;
  onChange: (() => Promise<void>) | undefined;

  constructor(
    private readonly select: (directory: string) => Promise<Selection>,
    private readonly create: (selection: Selection) => C,
  ) {}

  selectorRoots(key: string): readonly string[] {
    const selectedRoot = this.entries.get(key)?.selection.root;
    if (!selectedRoot) return [];
    const root = canonicalFile(selectedRoot);
    const roots = new Set([root, selectedRoot]);
    for (const [directory, selection] of this.directories) {
      if (selection.key !== key) continue;
      const relative = path.relative(root, canonicalFile(directory));
      if (!inside(root, canonicalFile(directory))) continue;
      let alias = directory;
      for (const _segment of relative.split(path.sep).filter(Boolean)) alias = path.dirname(alias);
      roots.add(alias);
    }
    return [...roots];
  }
  hostPath(key: string, file: string): string {
    const roots = this.selectorRoots(key), canonical = roots[0];
    if (!canonical || !inside(canonical, file)) return file;
    const alias = roots.find((root) => root !== canonical) ?? canonical;
    return path.join(alias, path.relative(canonical, file));
  }

  hasClient(client: C): boolean {
    return [...this.entries.values()].some((entry) => entry.lifecycle.client === client);
  }

  async open(directory: string): Promise<void> {
    await this.resolve(path.resolve(directory));
  }

  async refresh(manifest: string): Promise<boolean> {
    const changed = manifest === "" ? undefined : canonicalFile(manifest);
    const affected = [...this.generations.keys()].filter((directory) => {
      const previous = this.directories.get(directory);
      return changed === undefined || previous === undefined || previous.key === changed ||
        inside(path.dirname(changed), canonicalFile(directory));
    });
    // New/deleted roots can change keys. Pending discoveries have no key yet.
    const results = await Promise.all(affected.map((directory) => this.resolve(directory)));
    return results.some(Boolean);
  }

  /** The most specific discovered root owns a file; manifestless clients cover the remainder. */
  owns(key: string, file: string): boolean {
    file = canonicalFile(file);
    for (const directory of this.generations.keys()) {
      if (!this.directories.has(directory) && canonicalFile(directory) === path.dirname(file)) return false;
    }
    let owner = "<no-manifest>", length = -1;
    for (const { selection } of this.entries.values()) {
      if (selection.root === null) continue;
      const root = canonicalFile(selection.root);
      if (inside(root, file) && root.length > length) {
        owner = selection.key;
        length = root.length;
      }
    }
    return this.entries.has(key) && owner === key;
  }

  async removeFolder(folder: string): Promise<void> {
    for (const directory of this.generations.keys()) {
      if (inside(path.resolve(folder), directory)) {
        this.generations.delete(directory);
        this.directories.delete(directory);
      }
    }
    await this.enqueue(async () => {
      await this.prune();
      await this.onChange?.();
    });
  }

  async stop(): Promise<void> {
    this.stopped = true;
    this.generations.clear();
    await this.enqueue(async () => {
      await Promise.all([...this.entries.values()].map((entry) => entry.lifecycle.stop()));
      this.entries.clear();
      this.directories.clear();
    });
  }

  private async resolve(directory: string): Promise<boolean> {
    if (this.stopped) return false;
    const generation = ++this.sequence;
    this.generations.set(directory, generation);
    let selection = await this.select(directory);
    return this.enqueue(async () => {
      if (this.stopped || this.generations.get(directory) !== generation) return false;
      const latest = this.entries.get(selection.key);
      if (latest && latest.generation > generation) selection = latest.selection;
      this.directories.set(directory, selection);
      // Refresh every member mapping of this root, so an older member mapping
      // cannot restore a command that a later root selection has replaced.
      for (const [member, previous] of this.directories) {
        if (
          previous.key === selection.key || (selection.manifest !== null && selection.root !== null &&
            (previous.key.startsWith("folder:") || previous.key === "<no-manifest>") &&
            inside(selection.root, canonicalFile(member)) && (this.generations.get(member) ?? 0) <= generation)
        ) {
          this.directories.set(member, selection);
        }
      }
      let entry = this.entries.get(selection.key);
      let replaced = false;
      if (entry && entry.selection.command !== selection.command) {
        await entry.lifecycle.stop();
        this.entries.delete(selection.key);
        entry = undefined;
        replaced = true;
      }
      if (!entry) {
        entry = { generation, selection, lifecycle: new ClientLifecycle(() => this.create(selection)) };
        this.entries.set(selection.key, entry);
      }
      entry.generation = Math.max(entry.generation, generation);
      await this.prune();
      await this.onChange?.();
      // A newer selection may arrive while a previous client is stopping.
      if (this.stopped || this.generations.get(directory) !== generation) return false;
      await entry.lifecycle.startIfNeeded();
      await this.onChange?.();
      return replaced;
    });
  }
  private async prune(): Promise<void> {
    const used = new Set([...this.directories.values()].map((s) => s.key));
    for (const [key, entry] of this.entries) {
      if (!used.has(key)) {
        this.entries.delete(key);
        await entry.lifecycle.stop();
      }
    }
  }
  private enqueue<T>(work: () => Promise<T>): Promise<T> {
    const result = this.queue.then(work);
    this.queue = result.then(() => undefined, () => undefined);
    return result;
  }
}

/** MCP ordering depends on workspace folder order, independently of document discovery. */
export async function selectFirstWorkspaceFolder(
  folders: readonly string[],
  select: (directory: string) => Promise<Selection>,
): Promise<Selection | undefined> {
  return folders.length === 0 ? undefined : select(folders[0]);
}

function uriFile(uri: string): string | undefined {
  try {
    return fileURLToPath(uri);
  } catch {
    return undefined;
  }
}
function object(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null;
}

/** All document traffic and edit targets are checked at the protocol boundary. */
export function routingMiddleware(owns: (file: string) => boolean): Middleware {
  const allowedUri = (uri: string) => {
    const file = uriFile(uri);
    return file !== undefined && owns(file);
  };
  const allowedDocument = (document: { uri: { scheme: string; fsPath: string } }) =>
    document.uri.scheme === "file" && owns(document.uri.fsPath);
  const editsAllowed = (value: unknown): boolean => {
    if (!object(value)) return true;
    if (object(value.changes) && !Object.keys(value.changes).every(allowedUri)) return false;
    if (
      Array.isArray(value.documentChanges) && !value.documentChanges.every((change) => {
        if (!object(change)) return false;
        if (object(change.textDocument)) {
          return typeof change.textDocument.uri === "string" && allowedUri(change.textDocument.uri);
        }
        return [change.uri, change.oldUri, change.newUri].filter((uri) => uri !== undefined).every((uri) =>
          typeof uri === "string" && allowedUri(uri)
        );
      })
    ) return false;
    return true;
  };
  return {
    didOpen: async (document, next) => {
      if (allowedDocument(document)) await next(document);
    },
    didChange: async (event, next) => {
      if (allowedDocument(event.document)) await next(event);
    },
    didSave: async (document, next) => {
      if (allowedDocument(document)) await next(document);
    },
    // Closing an old overlay remains necessary after its ownership changes.
    didClose: async (document, next) => {
      await next(document);
    },
    handleDiagnostics: (uri, diagnostics, next) => {
      if (uri.scheme === "file" && owns(uri.fsPath)) next(uri, diagnostics);
    },
    sendRequest: async (type, param, token, next) => {
      if (
        object(param) && object(param.textDocument) && typeof param.textDocument.uri === "string" &&
        !allowedUri(param.textDocument.uri)
      ) throw new ResponseError(LSPErrorCodes.RequestCancelled, "Document ownership changed.");
      const result = await next(type, param, token);
      // Check again after awaiting: routing may have changed during the request.
      if (
        object(param) && object(param.textDocument) && typeof param.textDocument.uri === "string" &&
        !allowedUri(param.textDocument.uri)
      ) throw new ResponseError(LSPErrorCodes.RequestCancelled, "Document ownership changed.");
      const method = typeof type === "string" ? type : type.method;
      if (method === "textDocument/rename" && !editsAllowed(result)) {
        throw new ResponseError(LSPErrorCodes.RequestCancelled, "Document ownership changed.");
      }
      if (method === "textDocument/codeAction" && Array.isArray(result)) {
        return result.filter((action) => !object(action) || editsAllowed(action.edit)) as typeof result;
      }
      return result;
    },
    workspace: {
      handleApplyEdit: (params, next) =>
        editsAllowed(params.edit)
          ? next(params, CancellationToken.None)
          : { applied: false, failureReason: "Edit targets another RIDL root." },
    },
  };
}

/** Canonicalize identities, including unsaved/deleted paths, without discovering a RIDL root. */
export function canonicalFile(file: string): string {
  let current = normalizeAbsolutePath(file);
  const suffix: string[] = [];
  for (;;) {
    try {
      return normalizeAbsolutePath(path.join(fs.realpathSync.native(current), ...suffix));
    } catch {
      const parent = path.dirname(current);
      if (parent === current) return normalizeAbsolutePath(file);
      suffix.unshift(path.basename(current));
      current = parent;
    }
  }
}

/** Register owned-document providers through the library's public registration API. */
export function aliasRegistrations(
  capabilities: import("vscode-languageserver-protocol").ServerCapabilities,
  selector: ReturnType<typeof clientDocumentSelector>,
): {
  method: string;
  options: import("vscode-languageserver-protocol").TextDocumentRegistrationOptions & Record<string, unknown>;
}[] {
  const registrations: {
    method: string;
    options: import("vscode-languageserver-protocol").TextDocumentRegistrationOptions & Record<string, unknown>;
  }[] = [];
  const add = (method: string, options: Record<string, unknown> = {}) =>
    registrations.push({ method, options: { ...options, documentSelector: selector } });
  const sync = capabilities.textDocumentSync;
  if (typeof sync === "number" ? sync !== 0 : sync?.openClose) {
    add("textDocument/didOpen");
    add("textDocument/didClose");
  }
  const change = typeof sync === "number" ? sync : sync?.change;
  if (change) add("textDocument/didChange", { syncKind: change });
  const features = {
    "textDocument/completion": capabilities.completionProvider,
    "textDocument/hover": capabilities.hoverProvider,
    "textDocument/definition": capabilities.definitionProvider,
    "textDocument/references": capabilities.referencesProvider,
    "textDocument/rename": capabilities.renameProvider,
    "textDocument/formatting": capabilities.documentFormattingProvider,
    "textDocument/codeAction": capabilities.codeActionProvider,
    "textDocument/inlayHint": capabilities.inlayHintProvider,
    "textDocument/documentSymbol": capabilities.documentSymbolProvider,
  };
  for (const [method, capability] of Object.entries(features)) {
    if (capability) add(method, object(capability) ? capability : {});
  }
  return registrations;
}
