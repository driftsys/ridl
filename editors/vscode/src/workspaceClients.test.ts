import { strict as assert } from "node:assert";
import * as path from "node:path";
import { pathToFileURL } from "node:url";
import { test } from "node:test";
import { ClientPhase } from "./clientLifecycle";
import { Selection } from "./toolchainSelection";
import {
  aliasRegistrations,
  routingMiddleware,
  selectFirstWorkspaceFolder,
  WorkspaceClients,
} from "./workspaceClients";

const abs = (file: string) => path.resolve(file);
function selection(root: string | null, command = "bundled"): Selection {
  const manifest = root === null ? null : path.join(abs(root), "ridl.toml");
  return { key: manifest ?? "<no-manifest>", root: root === null ? null : abs(root), manifest, command };
}
class Client {
  starts = 0;
  stops = 0;
  state: ClientPhase = "stopped";
  constructor(readonly selected: Selection, readonly fail = false) {}
  phase() {
    return this.state;
  }
  async start() {
    this.starts++;
    if (this.fail) throw new Error("failed");
    this.state = "running";
  }
  async stop() {
    this.stops++;
    this.state = "stopped";
  }
}
function fixture(select: (directory: string) => Promise<Selection>, failFirst = false) {
  const clients: Client[] = [];
  const manager = new WorkspaceClients(select, (s) => {
    const c = new Client(s, failFirst && clients.length === 0);
    clients.push(c);
    return c;
  });
  return { manager, clients };
}
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

test("two_ridl_roots_inside_one_folder_get_two_clients", async () => {
  const f = fixture(async (d) => selection(d));
  await f.manager.open(abs("/work/a"));
  await f.manager.open(abs("/work/b"));
  assert.equal(f.clients.length, 2);
  assert.deepEqual(f.clients.map((c) => c.selected.root), [abs("/work/a"), abs("/work/b")]);
});
test("documents_with_same_manifest_share_one_client", async () => {
  const f = fixture(async () => selection("/work"));
  await Promise.all([f.manager.open(abs("/work/src")), f.manager.open(abs("/work/tests"))]);
  assert.equal(f.clients.length, 1);
});
test("manifestless_documents_share_one_client", async () => {
  const f = fixture(async () => selection(null));
  await f.manager.open(abs("/a"));
  await f.manager.open(abs("/b"));
  assert.equal(f.clients.length, 1);
});
test("concurrent_opens_and_pin_changes_use_latest_selection", async () => {
  const old = deferred<Selection>();
  let calls = 0;
  const f = fixture(async () => ++calls === 1 ? old.promise : selection("/work", "new"));
  const opening = f.manager.open(abs("/work/src"));
  await f.manager.refresh(abs("/work/ridl.toml"));
  old.resolve(selection("/work", "old"));
  await opening;
  assert.deepEqual(f.clients.map((c) => c.selected.command), ["new"]);
});
test("pin_change_restarts_only_its_root_when_binary_changes", async () => {
  let command = "old";
  const f = fixture(async (d) => selection(d, d === abs("/a") ? command : "other"));
  await f.manager.open(abs("/a"));
  await f.manager.open(abs("/b"));
  assert.equal(await f.manager.refresh(abs("/a/ridl.toml")), false);
  command = "new";
  assert.equal(await f.manager.refresh(abs("/a/ridl.toml")), true);
  assert.equal(f.clients[0].stops, 1);
  assert.equal(f.clients[1].stops, 0);
  assert.equal(f.clients.length, 3);
  assert.equal(await f.manager.refresh(abs("/a/member/ridl.toml")), false);
  assert.equal(f.clients.length, 3);
});
test("failed_start_retries_and_deactivation_stops_all_roots", async () => {
  const f = fixture(async (d) => selection(d), true);
  await f.manager.open(abs("/a"));
  await f.manager.open(abs("/a"));
  await f.manager.open(abs("/b"));
  assert.equal(f.clients.length, 3);
  await f.manager.stop();
  assert.deepEqual(f.clients.map((c) => c.stops), [0, 1, 1]);
});
test("stop_invalidates_pending_discovery", async () => {
  const pending = deferred<Selection>();
  const f = fixture(async () => pending.promise);
  const opening = f.manager.open(abs("/a"));
  await f.manager.stop();
  pending.resolve(selection("/a"));
  await opening;
  assert.equal(f.clients.length, 0);
});
test("mcp_uses_first_folder_selected_binary", async () => {
  const f = fixture(async (d) => selection(d, d === abs("/first") ? "first" : "second"));
  await f.manager.open(abs("/second"));
  await f.manager.open(abs("/first"));
  const result = await selectFirstWorkspaceFolder(
    [abs("/first"), abs("/second")],
    async (d) => selection(d, d === abs("/first") ? "first" : "second"),
  );
  assert.equal(result?.command, "first");
});

test("nested_and_manifestless_routing_is_exclusive_and_includes_root_toml", async () => {
  let nested = false;
  const f = fixture(async (d) =>
    d.startsWith(abs("/work/nested"))
      ? selection(nested ? "/work/nested" : "/work")
      : d.startsWith(abs("/work"))
      ? selection("/work")
      : selection(null)
  );
  await f.manager.open(abs("/loose"));
  await f.manager.open(abs("/work"));
  // The production routing API is declared before this red run.
  assert.equal(f.manager.owns(selection(null).key, abs("/work/file.ridl")), false);
  nested = true;
  await f.manager.open(abs("/work/nested/src"));
  assert.equal(f.manager.owns(selection("/work").key, abs("/work/nested/a.ridl")), false);
  assert.equal(f.manager.owns(selection("/work/nested").key, abs("/work/nested/ridl.toml")), true);
  assert.equal(f.manager.owns(selection("/work").key, abs("/work/a.ridl")), true);
  assert.equal(f.manager.owns(selection("/work").key, abs("/workspace/a.ridl")), false);
  nested = false;
  await f.manager.refresh(abs("/work/nested/ridl.toml"));
  assert.equal(f.clients.find((c) => c.selected.root === abs("/work/nested"))?.stops, 1);
  assert.equal(f.manager.owns(selection("/work").key, abs("/work/nested/a.ridl")), true);
});
test("root_creation_and_folder_removal_recompute_routing_and_stop_dropped_clients", async () => {
  let root: string | null = null;
  const f = fixture(async () => selection(root));
  await f.manager.open(abs("/work/src"));
  root = "/work";
  await f.manager.refresh(abs("/work/ridl.toml"));
  assert.equal(f.clients[0].stops, 1);
  assert.equal(f.manager.owns(selection("/work").key, abs("/work/src/a.ridl")), true);
  await f.manager.removeFolder(abs("/work"));
  assert.equal(f.clients[1].stops, 1);
  assert.equal(f.manager.owns(selection("/work").key, abs("/work/src/a.ridl")), false);
});
test("middleware_blocks_nested_open_change_diagnostics_requests_and_edits", async () => {
  const parent = abs("/work/a.ridl"), child = abs("/work/nested/b.ridl");
  const m = routingMiddleware((file) => file === parent);
  const doc = (file: string) =>
    ({ uri: { scheme: "file", fsPath: file, toString: () => pathToFileURL(file).href } }) as any;
  let opened = 0, changed = 0, published = 0, requested = 0, applied = 0;
  await m.didOpen?.(doc(child), async () => {
    opened++;
  });
  await m.didOpen?.(doc(parent), async () => {
    opened++;
  });
  await m.didChange?.({ document: doc(child) } as any, async () => {
    changed++;
  });
  await m.didChange?.({ document: doc(parent) } as any, async () => {
    changed++;
  });
  m.handleDiagnostics?.(doc(child).uri, [], () => {
    published++;
  });
  m.handleDiagnostics?.(doc(parent).uri, [], () => {
    published++;
  });
  const request = async () => {
    requested++;
    return { changes: { [pathToFileURL(child).href]: [] } };
  };
  await assert.rejects(async () =>
    m.sendRequest?.(
      "textDocument/rename",
      { textDocument: { uri: pathToFileURL(parent).href } },
      undefined,
      request as any,
    ), { code: -32800 });
  await assert.rejects(async () =>
    m.sendRequest?.(
      "textDocument/hover",
      { textDocument: { uri: pathToFileURL(child).href } },
      undefined,
      request as any,
    ), { code: -32800 });
  const rejected = await m.workspace?.handleApplyEdit?.(
    { edit: { changes: { [pathToFileURL(child).href]: [] } } },
    () => {
      applied++;
      return { applied: true };
    },
  );
  assert.equal((rejected as any)?.applied, false);
  assert.deepEqual([opened, changed, published, requested, applied], [1, 1, 1, 1, 0]);
});

test("folder_removal_and_reopen_cannot_revive_old_discovery", async () => {
  const old = deferred<Selection>();
  let calls = 0;
  const f = fixture(async () => ++calls === 1 ? old.promise : selection("/work", "new"));
  const first = f.manager.open(abs("/work/src"));
  await f.manager.removeFolder(abs("/work"));
  await f.manager.open(abs("/work/src"));
  old.resolve(selection("/work", "old"));
  await first;
  assert.deepEqual(f.clients.map((c) => c.selected.command), ["new"]);
});
test("older_discovery_for_another_member_cannot_restore_old_command", async () => {
  const old = deferred<Selection>();
  let calls = 0;
  const f = fixture(async () => ++calls === 1 ? old.promise : selection("/work", "new"));
  const first = f.manager.open(abs("/work/src"));
  await f.manager.open(abs("/work/tests"));
  old.resolve(selection("/work", "old"));
  await first;
  assert.deepEqual(f.clients.map((c) => c.selected.command), ["new"]);
});

test("valid_manifest_replaces_temporary_folder_routing", async () => {
  let calls = 0;
  const fallback: Selection = { key: `folder:${abs("/work")}`, manifest: null, root: abs("/work"), command: "bundled" };
  const f = fixture(async () => ++calls === 1 ? fallback : selection("/work", "pinned"));
  await f.manager.open(abs("/work/src"));
  await f.manager.open(abs("/work/tests"));
  assert.equal(f.clients[0].stops, 1);
  assert.equal(f.manager.owns(selection("/work").key, abs("/work/src/a.ridl")), true);
});
test("current_client_membership_survives_automatic_restart", async () => {
  const f = fixture(async (d) => selection(d));
  await f.manager.open(abs("/a"));
  f.clients[0].state = "starting";
  assert.equal(f.manager.hasClient(f.clients[0]), true);
  await f.manager.open(abs("/b"));
  assert.equal(f.manager.hasClient(f.clients[0]), true);
  f.clients[0].state = "running";
  await f.manager.removeFolder(abs("/a"));
  assert.equal(f.manager.hasClient(f.clients[0]), false);
});

test("canonical_roots_keep_symlink_documents_and_later_aliases", async () => {
  const fs = await import("node:fs");
  const os = await import("node:os");
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "ridl-editor-alias-"));
  try {
    const real = path.join(base, "real"), first = path.join(base, "first"), second = path.join(base, "second");
    fs.mkdirSync(real);
    fs.writeFileSync(path.join(real, "ridl.toml"), "[package]\nname='p'\n");
    fs.symlinkSync(real, first, "junction");
    fs.symlinkSync(real, second, "junction");
    const canonical = fs.realpathSync.native(real);
    const f = fixture(async () => selection(canonical, "pinned"));
    await f.manager.open(first);
    assert.equal(f.manager.owns(selection(canonical).key, path.join(first, "new.ridl")), true);
    assert.ok(f.manager.selectorRoots(selection(canonical).key).includes(first));
    await f.manager.open(second);
    assert.ok(f.manager.selectorRoots(selection(canonical).key).includes(second));
    assert.equal(f.clients.length, 1);
    await f.manager.stop();
  } finally {
    fs.rmSync(base, { recursive: true, force: true });
  }
});
test("alias_registrations_preserve_server_options_and_only_enabled_features", () => {
  const selector = [{ scheme: "file", language: "ridl", pattern: "/alias/**/*" }];
  const registrations = aliasRegistrations({
    textDocumentSync: { openClose: true, change: 2 },
    hoverProvider: true,
    completionProvider: { triggerCharacters: ["."] },
    renameProvider: { prepareProvider: true },
    codeActionProvider: { codeActionKinds: ["refactor.rewrite"] },
  }, selector);
  assert.deepEqual(registrations.map((r) => r.method), [
    "textDocument/didOpen",
    "textDocument/didClose",
    "textDocument/didChange",
    "textDocument/completion",
    "textDocument/hover",
    "textDocument/rename",
    "textDocument/codeAction",
  ]);
  assert.deepEqual(registrations.find((r) => r.method === "textDocument/completion")?.options, {
    triggerCharacters: ["."],
    documentSelector: selector,
  });
  assert.equal(registrations.find((r) => r.method === "textDocument/rename")?.options.prepareProvider, true);
  assert.deepEqual(registrations.find((r) => r.method === "textDocument/codeAction")?.options.codeActionKinds, [
    "refactor.rewrite",
  ]);
});

test("parent_cannot_claim_new_document_while_its_root_is_being_discovered", async () => {
  const pending = deferred<Selection>();
  let calls = 0;
  const f = fixture(async () => ++calls === 1 ? selection("/work") : pending.promise);
  await f.manager.open(abs("/work"));
  const opening = f.manager.open(abs("/work/nested"));
  assert.equal(f.manager.owns(selection("/work").key, abs("/work/nested/new.ridl")), false);
  pending.resolve(selection("/work/nested"));
  await opening;
  assert.equal(f.manager.owns(selection("/work/nested").key, abs("/work/nested/new.ridl")), true);
});

test("manifest_refresh_reselects_only_affected_mappings", async () => {
  const probes: string[] = [];
  const f = fixture(async (d) => {
    probes.push(d);
    return selection(d);
  });
  await f.manager.open(abs("/a"));
  await f.manager.open(abs("/b"));
  probes.splice(0);
  await f.manager.refresh(abs("/a/ridl.toml"));
  assert.deepEqual(probes, [abs("/a")]);
});

test("explicit_folder_override_also_routes_symlink_documents", async () => {
  const fs = await import("node:fs"), os = await import("node:os");
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "ridl-override-alias-"));
  try {
    const real = path.join(base, "real"), alias = path.join(base, "alias");
    fs.mkdirSync(real);
    fs.symlinkSync(real, alias, "junction");
    const selected: Selection = { key: `folder:${alias}`, manifest: null, root: alias, command: "custom" };
    const f = fixture(async () => selected);
    await f.manager.open(alias);
    assert.equal(f.manager.owns(selected.key, path.join(alias, "new.ridl")), true);
    await f.manager.stop();
  } finally {
    fs.rmSync(base, { recursive: true, force: true });
  }
});

test("late_discovery_with_an_obsolete_manifest_key_cannot_replace_the_latest_root", async () => {
  const old = deferred<Selection>();
  let calls = 0;
  const f = fixture(async () => ++calls === 1 ? old.promise : selection("/work/nested", "new"));
  const opening = f.manager.open(abs("/work/nested/src"));
  await f.manager.refresh(abs("/work/nested/ridl.toml"));
  old.resolve(selection("/work", "old"));
  await opening;
  assert.deepEqual(f.clients.map((c) => c.selected.command), ["new"]);
  assert.equal(f.clients[0].stops, 0);
  assert.equal(f.manager.owns(selection("/work/nested").key, abs("/work/nested/src/a.ridl")), true);
});

test("selection_superseded_while_stopping_does_not_start_its_obsolete_key", async () => {
  let selected = selection("/work", "original");
  const f = fixture(async () => selected);
  await f.manager.open(abs("/work/nested/src"));
  const stopping = deferred<void>(), releaseStop = deferred<void>();
  const first = f.clients[0];
  const stop = first.stop.bind(first);
  first.stop = async () => {
    stopping.resolve();
    await releaseStop.promise;
    await stop();
  };
  selected = selection("/work/nested", "obsolete");
  const replacing = f.manager.refresh(abs("/work/nested/ridl.toml"));
  await stopping.promise;
  selected = selection("/work/nested/src", "latest");
  const latest = f.manager.refresh(abs("/work/nested/ridl.toml"));
  releaseStop.resolve();
  await Promise.all([replacing, latest]);
  assert.deepEqual(f.clients.map((c) => c.selected.command), ["original", "latest"]);
  assert.equal(f.clients[1].starts, 1);
});
