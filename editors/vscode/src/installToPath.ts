// The "Install ridl to PATH" command: copies the bundled binary to a
// directory on the user's PATH — ~/.local/bin on POSIX platforms,
// %LOCALAPPDATA%\Programs\ridl on Windows — so Claude Code, Codex, and a
// terminal can run `ridl` without a second download. Pure planning and
// filesystem work — no `vscode` import; extension.ts supplies the
// notifications.

import { randomUUID } from "node:crypto";
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

/**
 * Copies through a temporary file in `targetDir`, then renames it over
 * `target`. A direct `copyFile` onto `target` truncates the destination
 * before writing it, which fails while a previous copy of the binary is
 * still running and can leave a truncated file on PATH if the copy is
 * interrupted. The rename is atomic on POSIX and replaces the old file
 * only once the new one is complete.
 */
export async function performCopy(plan: InstallPlan): Promise<void> {
  await fs.mkdir(plan.targetDir, { recursive: true });
  const tempPath = path.join(plan.targetDir, `.${plan.binaryName}.${randomUUID()}.tmp`);
  try {
    await fs.copyFile(plan.source, tempPath);
    if (plan.chmod) await fs.chmod(tempPath, 0o755);
    await fs.rename(tempPath, plan.target);
  } catch (error) {
    await fs.rm(tempPath, { force: true });
    throw error;
  }
}

/** `ridl --version` prints `ridl <version>`; returns `<version>`. */
export function parseVersionOutput(stdout: string): string | undefined {
  const parts = stdout.trim().split(/\s+/);
  return parts.length >= 2 ? parts[1] : undefined;
}

interface ParsedVersion {
  readonly major: number;
  readonly minor: number;
  readonly patch: number;
  readonly prerelease: string | undefined;
}

// Matches `0.2.0`, `editor-v0.2.0`, and either with a `-<prerelease>` suffix
// such as `editor-v0.0.0-install-check`.
const VERSION_PATTERN = /^(?:editor-v)?(\d+)\.(\d+)\.(\d+)(?:-(.+))?$/;

function parseVersion(version: string | undefined): ParsedVersion | undefined {
  if (version === undefined) return undefined;
  const match = VERSION_PATTERN.exec(version);
  if (!match) return undefined;
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
    prerelease: match[4],
  };
}

/**
 * Orders two parsed versions: negative when `a` is older than `b`, positive
 * when newer, zero when equal. The major.minor.patch triple is compared
 * numerically first. At equal triples, a prerelease sorts older than the
 * plain release of the same triple (matching semver precedence); two
 * prereleases of the same triple compare by the prerelease text.
 */
function compareVersions(a: ParsedVersion, b: ParsedVersion): number {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  if (a.patch !== b.patch) return a.patch - b.patch;
  if (a.prerelease === b.prerelease) return 0;
  if (a.prerelease === undefined) return 1;
  if (b.prerelease === undefined) return -1;
  return a.prerelease < b.prerelease ? -1 : 1;
}

/**
 * Whether the installed copy should be refreshed: both versions must parse,
 * and the installed one must be strictly older than the bundled one. A
 * newer installed copy (for example, one placed there by install.sh) is
 * left alone rather than offered for a downgrade; an unparseable version on
 * either side is treated as not stale.
 */
export function copyIsStale(
  bundledVersion: string | undefined,
  installedVersion: string | undefined,
): boolean {
  const bundled = parseVersion(bundledVersion);
  const installed = parseVersion(installedVersion);
  if (!bundled || !installed) return false;
  return compareVersions(installed, bundled) < 0;
}
