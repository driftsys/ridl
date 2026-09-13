// The "Install ridl to PATH" command: copies the bundled binary to a
// directory on the user's PATH — ~/.local/bin on POSIX platforms,
// %LOCALAPPDATA%\Programs\ridl on Windows — so Claude Code, Codex, and a
// terminal can run `ridl` without a second download. Pure planning and
// filesystem work — no `vscode` import; extension.ts supplies the
// notifications.

import { promises as fs } from "node:fs";
import * as path from "node:path";
import { bundledBinaryPath, BINARY_NAME } from "./binaryResolution";

export interface InstallPlan {
  readonly source: string;
  readonly targetDir: string;
  readonly target: string;
  readonly binaryName: string;
  readonly chmod: boolean;
}

export function planInstall(input: {
  platform: NodeJS.Platform;
  homedir: string;
  extensionPath: string;
}): InstallPlan {
  const binaryName = input.platform === "win32" ? `${BINARY_NAME}.exe` : BINARY_NAME;
  const targetDir =
    input.platform === "win32"
      ? path.join(input.homedir, "AppData", "Local", "Programs", "ridl")
      : path.join(input.homedir, ".local", "bin");
  return {
    binaryName,
    source: bundledBinaryPath(input.extensionPath, input.platform),
    targetDir,
    target: path.join(targetDir, binaryName),
    chmod: input.platform !== "win32",
  };
}

export function isDirOnPath(
  targetDir: string,
  pathEnv: string | undefined,
  delimiter: string,
  platform: NodeJS.Platform,
): boolean {
  if (!pathEnv) return false;
  const norm = (entry: string) => (platform === "win32" ? entry.trim().toLowerCase() : entry.trim());
  return pathEnv.split(delimiter).map(norm).includes(norm(targetDir));
}

/** The line that adds `targetDir` to PATH. On Windows, through the user
 * environment in the registry — `setx` truncates a PATH longer than 1024
 * characters. */
export function pathHint(targetDir: string, platform: NodeJS.Platform): string {
  return platform === "win32"
    ? `[Environment]::SetEnvironmentVariable("Path", "${targetDir};" + [Environment]::GetEnvironmentVariable("Path", "User"), "User")`
    : `export PATH="${targetDir}:$PATH"`;
}

export async function performCopy(plan: InstallPlan): Promise<void> {
  await fs.mkdir(plan.targetDir, { recursive: true });
  await fs.copyFile(plan.source, plan.target);
  if (plan.chmod) await fs.chmod(plan.target, 0o755);
}

/** `ridl --version` prints `ridl <version>`; returns `<version>`. */
export function parseVersionOutput(stdout: string): string | undefined {
  const parts = stdout.trim().split(/\s+/);
  return parts.length >= 2 ? parts[1] : undefined;
}

export function copyIsStale(
  bundledVersion: string | undefined,
  installedVersion: string | undefined,
): boolean {
  return bundledVersion !== undefined && installedVersion !== undefined && bundledVersion !== installedVersion;
}
