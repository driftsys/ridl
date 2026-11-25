import { strict as assert } from "node:assert";
import { test } from "node:test";
import { INSTALL, MANAGE_TRUST, missingMessage, RestrictedPrompts } from "./restrictedMode";
import type { WhichReport } from "./toolchainSelection";

const BUNDLED = "/ext/bin/ridl";

function missing(pin: string, manifest: string): WhichReport {
  return { schema: 1, manifest, running: "0.24.0", status: "missing", pin, path: null, source: null, diagnostic: null };
}

function harness(reports: Record<string, WhichReport>, bundled: string | null = BUNDLED) {
  const whichCommands: string[] = [];
  const installs: [string, string, string][] = [];
  const prompts: [string, readonly string[]][] = [];
  const answers: ((button: string | undefined) => void)[] = [];
  let trustCalls = 0;
  const prompter = new RestrictedPrompts(
    {
      async which(command: string, directory: string) {
        whichCommands.push(command);
        return reports[directory];
      },
      async install(command: string, pin: string, directory: string) {
        installs.push([command, pin, directory]);
      },
    },
    bundled ?? undefined,
    {
      prompt: (message, buttons) =>
        new Promise((resolve) => {
          prompts.push([message, buttons]);
          answers.push(resolve);
        }),
      async manageTrust() {
        trustCalls++;
      },
      log() {},
    },
  );
  return { prompter, whichCommands, installs, prompts, answers, trust: () => trustCalls };
}

test("restricted_missing_release_prompts_without_installing", async () => {
  const h = harness({ "/a": missing("0.23.1", "/a/ridl.toml") });
  await h.prompter.check("/a");
  assert.deepEqual(h.prompts, [[missingMessage("0.23.1"), [INSTALL, MANAGE_TRUST]]]);
  assert.deepEqual(h.installs, []);
  assert.ok(h.whichCommands.every((command) => command === BUNDLED));
});

test("restricted_prompt_is_shown_once_per_release_across_roots", async () => {
  const h = harness({
    "/a": missing("0.23.1", "/a/ridl.toml"),
    "/b": missing("0.23.1", "/b/ridl.toml"),
  });
  await h.prompter.check("/a");
  await h.prompter.check("/b");
  await h.prompter.check("/a");
  await h.prompter.check("/b");
  // Answer every pending prompt, so a regression fails on the counts below.
  for (const answer of h.answers) answer(INSTALL);
  await h.prompter.settled();
  await h.prompter.check("/a");
  await h.prompter.check("/b");
  for (const answer of h.answers) answer(INSTALL);
  await h.prompter.settled();
  assert.equal(h.prompts.length, 1);
  assert.equal(h.installs.length, 1);
  assert.deepEqual(h.installs[0], [BUNDLED, "0.23.1", "/a"]);
});

test("restricted_manage_trust_opens_the_trust_editor", async () => {
  const h = harness({ "/a": missing("0.23.1", "/a/ridl.toml") });
  await h.prompter.check("/a");
  h.answers[0](MANAGE_TRUST);
  await h.prompter.settled();
  assert.equal(h.trust(), 1);
  assert.deepEqual(h.installs, []);
});

test("restricted_dismissed_prompt_installs_nothing", async () => {
  const h = harness({ "/a": missing("0.23.1", "/a/ridl.toml") });
  await h.prompter.check("/a");
  h.answers[0](undefined);
  await h.prompter.settled();
  assert.deepEqual(h.installs, []);
  assert.equal(h.trust(), 0);
});

test("restricted_without_a_bundle_runs_nothing", async () => {
  const h = harness({ "/a": missing("0.23.1", "/a/ridl.toml") }, null);
  await h.prompter.check("/a");
  await h.prompter.check("/a");
  assert.deepEqual(h.whichCommands, []);
  assert.deepEqual(h.prompts, []);
});

test("restricted_compatible_and_invalid_do_not_prompt", async () => {
  const h = harness({
    "/ok": {
      schema: 1,
      manifest: "/ok/ridl.toml",
      running: "0.23.0",
      status: "compatible",
      pin: "0.23.0",
      path: "/x/ridl",
      source: "installed",
      diagnostic: null,
    },
    "/loose": {
      schema: 1,
      manifest: null,
      running: "0.23.0",
      status: "unpinned",
      pin: null,
      path: "/x/ridl",
      source: "running",
      diagnostic: null,
    },
    "/bad": {
      schema: 1,
      manifest: "/bad/ridl.toml",
      running: "0.23.0",
      status: "invalid",
      pin: null,
      path: null,
      source: null,
      diagnostic: { code: "MANI-024", message: "bad" },
    },
  });
  for (const directory of ["/ok", "/loose", "/bad"]) await h.prompter.check(directory);
  assert.deepEqual(h.prompts, []);
  assert.deepEqual(h.installs, []);
});
