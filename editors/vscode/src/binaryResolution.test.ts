import { strict as assert } from "node:assert";
import * as path from "node:path";
import { test } from "node:test";
import {
  bundledBinaryPath,
  isLegacyServerName,
  resolveBinary,
  resolveLspCommand,
  resolveMcpDefinition,
  shouldStartClientForLanguage,
} from "./binaryResolution";

const EXT = "/fake/extensions/driftsys.ridl-vscode-0.1.0";

test("tier 1: an explicit setting wins even when a bundled binary exists", () => {
  const resolved = resolveBinary({
    configuredPath: "/opt/ridl/bin/ridl",
    extensionPath: EXT,
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
    platform: "linux",
    exists: () => false,
  });
  assert.deepEqual(resolved, { command: "ridl", source: "path" });
});

test("a blank setting counts as unset", () => {
  const resolved = resolveBinary({
    configuredPath: "   ",
    extensionPath: EXT,
    platform: "linux",
    exists: () => false,
  });
  assert.equal(resolved.source, "path");
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
    platform: "linux",
    exists: (file) => file === bundled,
  });
  assert.deepEqual(resolved, { command: bundled, args: ["lsp"] });
});

test("the language client starts for typl and ridl documents, not others", () => {
  assert.equal(shouldStartClientForLanguage("typl"), true);
  assert.equal(shouldStartClientForLanguage("ridl"), true);
  assert.equal(shouldStartClientForLanguage("plaintext"), false);
  assert.equal(shouldStartClientForLanguage("markdown"), false);
  assert.equal(shouldStartClientForLanguage(""), false);
});
