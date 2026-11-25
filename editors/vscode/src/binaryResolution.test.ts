import { strict as assert } from "node:assert";
import * as path from "node:path";
import { test } from "node:test";
import {
  bundledBinaryPath,
  clientDocumentSelector,
  findOnPath,
  isLegacyServerName,
  normalizeAbsolutePath,
  resolveBinary,
  resolveLspCommand,
  resolveMcpDefinition,
  shouldStartClientForLanguage,
} from "./binaryResolution";

const EXT = "/fake/extensions/driftsys.ridl-lang-0.1.0";

test("tier 1: an explicit setting wins even when a bundled binary exists", () => {
  const resolved = resolveBinary({
    configuredPath: "/opt/ridl/bin/ridl",
    extensionPath: EXT,
    pathValue: undefined,
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
    pathValue: undefined,
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
    pathValue: "/usr/bin:/opt/ridl",
    platform: "linux",
    exists: (file) => file === "/opt/ridl/ridl",
  });
  assert.deepEqual(resolved, { command: "/opt/ridl/ridl", source: "path" });
});

test("path_lookup_uses_only_absolute_entries", () => {
  assert.equal(
    findOnPath(".:bin::/abs", "linux", (f) => f !== "/abs/ridl" ? f === "bin/ridl" || f === "ridl" : true),
    "/abs/ridl",
  );
  assert.equal(
    findOnPath(".;C:\\tools;relative", "win32", (f) => f === "C:\\tools\\ridl.exe" || f === "ridl.exe"),
    "C:\\tools\\ridl.exe",
  );
  assert.equal(findOnPath(".;relative", "win32", () => true), undefined);
  assert.equal(findOnPath(undefined, "linux", () => true), undefined);
  // Drive-less rooted and drive-relative Windows entries depend on the current drive or directory.
  assert.equal(findOnPath("\\tools;C:tools;\\\\host\\share\\bin", "win32", () => true), "\\\\host\\share\\bin\\ridl.exe");
  assert.equal(findOnPath("\\tools;C:tools", "win32", () => true), undefined);
  assert.equal(findOnPath("C:/tools", "win32", () => true), "C:\\tools\\ridl.exe");
});

test("path_lookup_skips_candidates_that_are_not_usable", () => {
  // A directory or a non-executable file named ridl earlier on PATH is skipped.
  const usable = (file: string) => file === "/c/ridl";
  assert.equal(findOnPath("/a:/b:/c", "linux", usable), "/c/ridl");
  const resolved = resolveBinary({
    configuredPath: undefined,
    extensionPath: EXT,
    pathValue: "/a:/c",
    platform: "linux",
    exists: () => true,
    usable,
  });
  assert.deepEqual(resolved, { command: bundledBinaryPath(EXT, "linux"), source: "bundled" });
  assert.deepEqual(
    resolveBinary({
      configuredPath: undefined,
      extensionPath: EXT,
      pathValue: "/a:/c",
      platform: "linux",
      exists: (file) => file !== bundledBinaryPath(EXT, "linux"),
      usable,
    }),
    { command: "/c/ridl", source: "path" },
  );
  const base = {
    configuredPath: undefined,
    extensionPath: EXT,
    pathValue: undefined,
    platform: "linux" as const,
  };
  assert.deepEqual(resolveBinary({ ...base, pathValue: ".", exists: () => false }), {
    command: bundledBinaryPath(base.extensionPath, base.platform),
    source: "none",
  });
});

test("a blank setting counts as unset", () => {
  const resolved = resolveBinary({
    configuredPath: "   ",
    extensionPath: EXT,
    pathValue: undefined,
    platform: "linux",
    exists: () => false,
  });
  assert.equal(resolved.source, "none");
});

test("the old ridl-lsp binary name is recognised so the user gets a warning", () => {
  assert.equal(isLegacyServerName("/home/dev/.cargo/bin/ridl-lsp"), true);
  assert.equal(isLegacyServerName("C:\\Users\\dev\\.cargo\\bin\\ridl-lsp.exe"), true);
  assert.equal(isLegacyServerName("/home/dev/.cargo/bin/ridl"), false);
});

test("the MCP definition spawns the same binary with the mcp subcommand", () => {
  const bundled = path.join(EXT, "bin", "ridl");
  const definition = resolveMcpDefinition({
    configuredPath: undefined,
    extensionPath: EXT,
    pathValue: undefined,
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

test("the LSP command spawns the same binary with the lsp subcommand", () => {
  const bundled = path.join(EXT, "bin", "ridl");
  const resolved = resolveLspCommand({
    configuredPath: undefined,
    extensionPath: EXT,
    pathValue: undefined,
    platform: "linux",
    exists: (file) => file === bundled,
  });
  assert.deepEqual(resolved, { command: bundled, args: ["lsp"] });
});

test("the language client starts for typl, ridl and rsdl documents, not others", () => {
  assert.equal(shouldStartClientForLanguage("typl"), true);
  assert.equal(shouldStartClientForLanguage("ridl"), true);
  assert.equal(shouldStartClientForLanguage("rsdl"), true);
  assert.equal(shouldStartClientForLanguage("plaintext"), false);
  assert.equal(shouldStartClientForLanguage("markdown"), false);
  assert.equal(shouldStartClientForLanguage(""), false);
});

test("the language client is scoped to typl, ridl and rsdl files", () => {
  assert.deepEqual(clientDocumentSelector(), [
    { scheme: "file", language: "typl" },
    { scheme: "file", language: "ridl" },
    { scheme: "file", language: "rsdl" },
  ]);
});

test("root_selectors_exclude_nested_clients", () => {
  const root = path.resolve("/work/a[1]{x}");
  const selectors = clientDocumentSelector(root, [path.join(root, "nested")]);
  assert.equal(selectors.length, 4);
  assert.equal(selectors[0].pattern, root.replace(/\\/g, "/").replace(/[\[\]{}*?!()]/g, "[$&]") + "/**/*");
  assert.deepEqual(selectors[3], {
    scheme: "file",
    language: "*",
    pattern: root.replace(/\\/g, "/").replace(/[\[\]{}*?!()]/g, "[$&]") + "/ridl.toml",
  });
});
test("mcp_selected_command_passes_through", () => {
  assert.equal(
    resolveMcpDefinition({
      configuredPath: undefined,
      extensionPath: EXT,
      pathValue: undefined,
      platform: "linux",
      exists: () => true,
      extensionVersion: "1.0.0",
      workspaceFolder: "/work",
      selectedCommand: "/selected/ridl",
    }).command,
    "/selected/ridl",
  );
});

test("windows_verbatim_paths_match_editor_drive_and_unc_paths", () => {
  assert.equal(normalizeAbsolutePath("\\\\?\\C:\\work\\ridl.toml", "win32"), "C:\\work\\ridl.toml");
  assert.equal(normalizeAbsolutePath("\\\\?\\UNC\\host\\share\\ridl.toml", "win32"), "\\\\host\\share\\ridl.toml");
});
