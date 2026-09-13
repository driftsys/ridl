import { strict as assert } from "node:assert";
import { promises as fs } from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { test } from "node:test";
import {
  copyIsStale,
  isDirOnPath,
  parseVersionOutput,
  pathHint,
  performCopy,
  planInstall,
} from "./installToPath";

const EXT = "/fake/extensions/driftsys.ridl-vscode-0.1.0";
const HOME = "/home/dev";

test("planInstall on Linux: ~/.local/bin/ridl, chmod", () => {
  const plan = planInstall({ platform: "linux", homedir: HOME, extensionPath: EXT });
  assert.equal(plan.binaryName, "ridl");
  assert.equal(plan.source, path.join(EXT, "bin", "ridl"));
  assert.equal(plan.targetDir, path.join(HOME, ".local", "bin"));
  assert.equal(plan.target, path.join(HOME, ".local", "bin", "ridl"));
  assert.equal(plan.chmod, true);
});

test("planInstall on Windows: ridl.exe, no chmod", () => {
  const plan = planInstall({ platform: "win32", homedir: "C:\\Users\\dev", extensionPath: EXT });
  assert.equal(plan.binaryName, "ridl.exe");
  assert.equal(plan.chmod, false);
  assert.equal(plan.targetDir, path.join("C:\\Users\\dev", "AppData", "Local", "Programs", "ridl"));
  assert.equal(
    plan.target,
    path.join("C:\\Users\\dev", "AppData", "Local", "Programs", "ridl", "ridl.exe"),
  );
});

test("isDirOnPath is exact on POSIX and case-insensitive on Windows", () => {
  assert.equal(isDirOnPath("/home/dev/.local/bin", "/usr/bin:/home/dev/.local/bin", ":", "linux"), true);
  assert.equal(isDirOnPath("/home/dev/.local/bin", "/usr/bin:/home/dev/.local/BIN", ":", "linux"), false);
  assert.equal(isDirOnPath("C:\\Users\\dev\\bin", "C:\\Windows;c:\\users\\DEV\\bin", ";", "win32"), true);
  assert.equal(isDirOnPath("/x", undefined, ":", "linux"), false);
});

test("pathHint prints a shell line per platform", () => {
  assert.equal(pathHint("/home/dev/.local/bin", "darwin"), 'export PATH="/home/dev/.local/bin:$PATH"');
  assert.match(pathHint("C:\\Users\\dev\\bin", "win32"), /Path/);
});

test("performCopy creates the directory, copies, and sets the mode", async () => {
  const dir = await fs.mkdtemp(path.join(os.tmpdir(), "ridl-install-"));
  try {
    const source = path.join(dir, "ridl");
    await fs.writeFile(source, "#!/bin/sh\necho ridl\n");
    const plan = {
      source,
      targetDir: path.join(dir, "target", "bin"),
      target: path.join(dir, "target", "bin", "ridl"),
      binaryName: "ridl",
      chmod: process.platform !== "win32",
    };
    await performCopy(plan);
    const copied = await fs.readFile(plan.target, "utf8");
    assert.equal(copied, "#!/bin/sh\necho ridl\n");
    if (plan.chmod) {
      const mode = (await fs.stat(plan.target)).mode & 0o777;
      assert.equal(mode, 0o755);
    }
  } finally {
    await fs.rm(dir, { recursive: true, force: true });
  }
});

test("parseVersionOutput takes the token after the program name", () => {
  assert.equal(parseVersionOutput("ridl editor-v0.1.0\n"), "editor-v0.1.0");
  assert.equal(parseVersionOutput("ridl 0.0.0"), "0.0.0");
  assert.equal(parseVersionOutput(""), undefined);
});

test("copyIsStale only when both versions are known and differ", () => {
  assert.equal(copyIsStale("editor-v0.2.0", "editor-v0.1.0"), true);
  assert.equal(copyIsStale("editor-v0.2.0", "editor-v0.2.0"), false);
  assert.equal(copyIsStale(undefined, "editor-v0.1.0"), false);
  assert.equal(copyIsStale("editor-v0.2.0", undefined), false);
});
