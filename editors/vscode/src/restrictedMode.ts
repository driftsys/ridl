// Prompts in Restricted Mode. Pure — no `vscode` import — so it is unit-tested directly.
import type { ToolchainSelector } from "./toolchainSelection";

export const INSTALL = "Install";
export const MANAGE_TRUST = "Manage Workspace Trust";

export function missingMessage(pin: string): string {
  return `This workspace requires ridl ${pin}, which is not installed.`;
}

export interface RestrictedEffects {
  prompt(message: string, buttons: readonly string[]): Promise<string | undefined>;
  manageTrust(): Promise<void>;
  log(message: string): void;
}

export class RestrictedPrompts {
  private readonly prompted = new Set<string>();
  private readonly installing = new Set<string>();
  private readonly answers = new Set<Promise<void>>();
  private loggedNoBundle = false;

  constructor(
    private readonly selector: Pick<ToolchainSelector, "which" | "install">,
    private readonly bundled: string | undefined,
    private readonly effects: RestrictedEffects,
  ) {}

  /** Report whether the pinned release is missing, with the bundled binary only. */
  async check(directory: string): Promise<void> {
    if (this.bundled === undefined) {
      if (!this.loggedNoBundle) {
        this.loggedNoBundle = true;
        this.effects.log("Restricted Mode: the extension has no bundled ridl binary, so no release check runs.");
      }
      return;
    }
    try {
      const report = await this.selector.which(this.bundled, directory);
      if (report.status !== "missing" || this.prompted.has(report.pin)) return;
      this.prompted.add(report.pin);
      const pin = report.pin;
      // The answer is not awaited, so activation never waits for the user.
      const answer = this.answer(this.bundled, pin, directory).finally(() => this.answers.delete(answer));
      this.answers.add(answer);
    } catch (error) {
      this.effects.log(`Restricted Mode: ${String(error)}`);
    }
  }

  /** Resolve when every pending answer handler has finished. */
  async settled(): Promise<void> {
    while (this.answers.size > 0) await Promise.all([...this.answers]);
  }

  private async answer(bundled: string, pin: string, directory: string): Promise<void> {
    try {
      const button = await this.effects.prompt(missingMessage(pin), [INSTALL, MANAGE_TRUST]);
      if (button === INSTALL && !this.installing.has(pin)) {
        this.installing.add(pin);
        await this.selector.install(bundled, pin, directory);
      } else if (button === MANAGE_TRUST) {
        await this.effects.manageTrust();
      }
    } catch (error) {
      this.effects.log(`Restricted Mode: ${String(error)}`);
    }
  }
}
