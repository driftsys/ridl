import * as path from "node:path";
import { BinarySource, normalizeAbsolutePath } from "./binaryResolution";

interface WhichBase {
  readonly schema: 1;
  readonly manifest: string | null;
  readonly running: string;
}
export type WhichReport =
  & WhichBase
  & (
    | { status: "compatible"; pin: string; path: string; source: "installed" | "running"; diagnostic: null }
    | { status: "unpinned"; pin: null; path: string; source: "running"; diagnostic: null }
    | { status: "missing"; pin: string; path: null; source: null; diagnostic: null }
    | {
      status: "invalid";
      pin: string | null;
      path: null;
      source: null;
      diagnostic: { code: "MANI-024" | "MANI-025"; message: string };
    }
  );
export interface BinaryRunner {
  run(
    command: string,
    args: readonly string[],
    cwd: string,
  ): Promise<{ exitCode: number; stdout: string; stderr: string }>;
}
export interface Selection {
  readonly key: string;
  readonly manifest: string | null;
  readonly root: string | null;
  readonly command: string;
}
export interface SelectionEffects {
  progress<T>(title: string, work: () => Promise<T>): Promise<T>;
  offerInstall(pin: string): Promise<boolean>;
  errorOnce(key: string, message: string): void;
  log(message: string): void;
}
export interface SelectInput {
  readonly resolver: { command: string; source: BinarySource };
  readonly directory: string;
  readonly autoInstall: boolean;
}
function record(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
function release(value: unknown): value is string {
  return typeof value === "string" && /^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$/.test(value) &&
    value.split(".").every((part) => BigInt(part) <= 18446744073709551615n);
}
function pinnable(value: unknown): value is string {
  if (!release(value)) return false;
  const [major, minor] = value.split(".").map(BigInt);
  return major > 0n || minor >= 23n;
}
function absolute(value: unknown): value is string {
  return typeof value === "string" && !value.includes("\0") && path.isAbsolute(value);
}
/** Validate the complete schema before a subprocess path or pin can be used. */
export function parseWhichReport(value: unknown): WhichReport | undefined {
  if (
    !record(value) || value.schema !== 1 || !release(value.running) ||
    !(value.manifest === null || absolute(value.manifest))
  ) return undefined;
  switch (value.status) {
    case "compatible":
      if (
        !absolute(value.manifest) || !pinnable(value.pin) || !absolute(value.path) ||
        (value.source !== "installed" && value.source !== "running") || value.diagnostic !== null
      ) return undefined;
      break;
    case "unpinned":
      if (value.pin !== null || !absolute(value.path) || value.source !== "running" || value.diagnostic !== null) {
        return undefined;
      }
      break;
    case "missing":
      if (
        !absolute(value.manifest) || !pinnable(value.pin) || value.path !== null || value.source !== null ||
        value.diagnostic !== null
      ) return undefined;
      break;
    case "invalid": {
      if (
        !absolute(value.manifest) || value.path !== null || value.source !== null || !record(value.diagnostic) ||
        typeof value.diagnostic.message !== "string"
      ) return undefined;
      const code = value.diagnostic.code;
      if (
        code === "MANI-024"
          ? value.pin !== null
          : code === "MANI-025"
          ? !release(value.pin) || pinnable(value.pin)
          : true
      ) return undefined;
      break;
    }
    default:
      return undefined;
  }
  return value as unknown as WhichReport;
}

export class ToolchainSelector {
  private readonly installs = new Map<string, Promise<void>>();
  private readonly notified = new Set<string>();
  constructor(private readonly runner: BinaryRunner, private readonly effects: SelectionEffects) {}

  async select(input: SelectInput): Promise<Selection> {
    const directory = path.resolve(input.directory);
    const fallback = { key: `folder:${directory}`, manifest: null, root: directory, command: input.resolver.command };
    if (input.resolver.source === "setting") return fallback;
    let report: WhichReport;
    try {
      report = await this.probe(input);
    } catch (error) {
      this.report("discovery", `Could not select a RIDL toolchain: ${String(error)}`);
      return fallback;
    }
    const selection = (command: string): Selection => ({
      key: report.manifest === null ? "<no-manifest>" : normalizeAbsolutePath(report.manifest),
      manifest: report.manifest === null ? null : normalizeAbsolutePath(report.manifest),
      root: report.manifest === null ? null : path.dirname(normalizeAbsolutePath(report.manifest)),
      command,
    });
    if (report.status === "compatible" || report.status === "unpinned") return selection(report.path);
    if (report.status === "invalid") return selection(input.resolver.command);
    const pin = report.pin;
    try {
      if (!input.autoInstall && !(await this.effects.offerInstall(pin))) return selection(input.resolver.command);
      let install = this.installs.get(pin);
      if (!install) {
        install = this.effects.progress(`Installing RIDL ${pin}`, async () => {
          const result = await this.runner.run(input.resolver.command, ["toolchain", "install", pin], directory);
          if (result.exitCode !== 0) throw new Error(result.stderr || `install exited ${result.exitCode}`);
        });
        this.installs.set(pin, install);
        // Remove only this operation, so a later document open can retry a failure.
        void install.then(() => this.installs.delete(pin), () => this.installs.delete(pin));
      }
      await install;
      const fresh = await this.probe(input);
      if (fresh.status !== "compatible" && fresh.status !== "unpinned") {
        throw new Error("toolchain is still unavailable after installation");
      }
      report = fresh;
      return selection(fresh.path);
    } catch (error) {
      this.report(`install:${pin}`, `Could not install RIDL ${pin}: ${String(error)}`);
      return selection(input.resolver.command);
    }
  }

  private async probe(input: SelectInput): Promise<WhichReport> {
    const result = await this.runner.run(
      input.resolver.command,
      ["toolchain", "which", "--json", input.directory],
      input.directory,
    );
    const report = parseWhichReport(JSON.parse(result.stdout));
    if (!report) throw new Error("unsupported or malformed toolchain which JSON");
    const expected = report.status === "compatible" || report.status === "unpinned" ? 0 : 1;
    if (result.exitCode !== expected) {
      throw new Error(`toolchain which exit ${result.exitCode} disagrees with ${report.status}`);
    }
    return report;
  }
  private report(key: string, message: string): void {
    this.effects.log(message);
    if (this.notified.has(key)) return;
    this.notified.add(key);
    this.effects.errorOnce(key, message);
  }
}
