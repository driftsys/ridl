import { strict as assert } from "node:assert";
import * as path from "node:path";
import { test } from "node:test";
import { parseWhichReport, SelectInput, ToolchainSelector } from "./toolchainSelection";

const directory = path.resolve("/work/project/src");
const manifest = path.resolve("/work/project/ridl.toml");
const bundled = path.resolve("/extension/bin/ridl");
const installed = path.resolve("/data/versions/0.23.0/ridl");
const compatible = {
  schema: 1,
  manifest,
  running: "0.24.0",
  pin: "0.23.0",
  status: "compatible",
  path: installed,
  source: "installed",
  diagnostic: null,
};
const missing = { ...compatible, status: "missing", path: null, source: null };
const unpinned = { ...compatible, status: "unpinned", pin: null, source: "running", path: bundled };
const invalid = { ...missing, status: "invalid", pin: null, diagnostic: { code: "MANI-024", message: "invalid pin" } };
const input: SelectInput = { resolver: { command: bundled, source: "bundled" }, directory, autoInstall: true };
function response(
  value: unknown,
  exitCode = (value as any)?.status === "missing" || (value as any)?.status === "invalid" ? 1 : 0,
) {
  return { exitCode, stdout: typeof value === "string" ? value : JSON.stringify(value), stderr: "failure details" };
}
function fixture(responses: (ReturnType<typeof response> | Error)[], accept = true) {
  const calls: unknown[] = [],
    errors: string[] = [],
    logs: string[] = [],
    offers: string[] = [],
    progress: string[] = [];
  const selector = new ToolchainSelector({
    async run(command, args, cwd) {
      calls.push([command, args, cwd]);
      const result = responses.shift();
      if (!result) throw new Error("unexpected runner call");
      if (result instanceof Error) throw result;
      return result;
    },
  }, {
    async progress(title, work) {
      progress.push(title);
      return work();
    },
    async offerInstall(pin) {
      offers.push(pin);
      return accept;
    },
    errorOnce(key, message) {
      errors.push(key);
    },
    log(message) {
      logs.push(message);
    },
  });
  return { selector, calls, errors, logs, offers, progress };
}

test("explicit_server_path_skips_toolchain_commands", async () => {
  const f = fixture([]);
  assert.equal(
    (await f.selector.select({ ...input, resolver: { command: "custom", source: "setting" } })).command,
    "custom",
  );
  assert.equal(f.calls.length, 0);
});
test("compatible_and_unpinned_start_selected_binary", async () => {
  const f = fixture([response(compatible), response(unpinned), response({ ...unpinned, manifest: null })]);
  assert.equal((await f.selector.select(input)).command, installed);
  assert.equal((await f.selector.select(input)).key, manifest);
  assert.equal((await f.selector.select(input)).key, "<no-manifest>");
  assert.deepEqual(f.calls[0], [bundled, ["toolchain", "which", "--json", directory], directory]);
});
test("missing_installs_requeries_then_starts", async () => {
  const f = fixture([response(missing), response(installed), response(compatible)]);
  const selected = await f.selector.select(input);
  assert.equal(selected.command, installed);
  assert.equal(selected.key, manifest);
  assert.deepEqual(f.calls, [
    [bundled, ["toolchain", "which", "--json", directory], directory],
    [bundled, ["toolchain", "install", "0.23.0"], directory],
    [bundled, ["toolchain", "which", "--json", directory], directory],
  ]);
  assert.equal(f.progress.length, 1);
});
test("auto_install_off_uses_install_button", async () => {
  for (const accept of [false, true]) {
    const f = fixture([response(missing), response(installed), response(compatible)], accept);
    assert.equal((await f.selector.select({ ...input, autoInstall: false })).command, accept ? installed : bundled);
    assert.deepEqual(f.offers, ["0.23.0"]);
    assert.equal(f.calls.length, accept ? 3 : 1);
  }
});
test("install_failure_uses_bundled_and_reports_once", async () => {
  const f = fixture([response(missing), response("", 2), response(missing), response("", 2)]);
  assert.equal((await f.selector.select(input)).command, bundled);
  assert.equal((await f.selector.select(input)).command, bundled);
  assert.equal(f.errors.length, 1);
  assert.equal(f.calls.length, 4, "a later retry is allowed");
});
test("unknown_schema_and_malformed_results_fall_back_once", async () => {
  for (
    const result of [
      response("not json"),
      response("unsupported command", 2),
      new Error("ENOENT"),
    ]
  ) {
    const f = fixture([result, result]);
    const selection = await f.selector.select(input);
    assert.equal(selection.command, bundled);
    assert.equal(selection.key, `folder:${directory}`);
    await f.selector.select(input);
    assert.equal(f.errors.length, 1);
    assert.ok(f.logs.length > 0);
    assert.equal(f.calls.length, 2);
  }
});
test("schema_one_validates_every_state_and_nullability", () => {
  for (
    const value of [compatible, unpinned, missing, invalid, {
      ...invalid,
      pin: "0.22.0",
      diagnostic: { code: "MANI-025", message: "below floor" },
    }]
  ) {
    assert.deepEqual(parseWhichReport(value), value);
    for (const key of Object.keys(value)) {
      const copy: any = { ...value };
      delete copy[key];
      assert.equal(parseWhichReport(copy), undefined, `required ${key}`);
    }
  }
  for (
    const value of [
      null,
      [],
      { ...compatible, schema: 2 },
      { ...compatible, path: 17 },
      { ...compatible, path: null },
      { ...compatible, path: "relative/ridl" },
      { ...compatible, manifest: "relative/ridl.toml" },
      { ...compatible, manifest: null },
      { ...compatible, status: "other" },
      { ...compatible, source: "other" },
      { ...compatible, source: ["installed"] },
      { ...compatible, source: { toString: "installed" } },
      { ...compatible, running: "v0.23.0" },
      { ...missing, pin: null },
      { ...missing, pin: "0.22.0" },
      { ...missing, pin: "0.23.0; command" },
      { ...invalid, diagnostic: { code: "MANI-026", message: "wrong" } },
      { ...invalid, pin: "0.23.0" },
      { ...invalid, diagnostic: { code: "MANI-025", message: "wrong" } },
      { ...unpinned, source: "installed" },
      { ...unpinned, pin: "0.23.0" },
    ]
  ) assert.equal(parseWhichReport(value), undefined, JSON.stringify(value));
  if (process.platform === "win32") {
    assert.ok(
      parseWhichReport({ ...compatible, manifest: "C:\\project\\ridl.toml", path: "\\\\host\\share\\ridl.exe" }),
    );
  }
});
test("selector_rejects_exit_status_disagreement_and_invalid_never_installs", async () => {
  for (const result of [response(compatible, 1), response(missing, 0), response(invalid, 0), response(unpinned, 2)]) {
    const f = fixture([result]);
    assert.equal((await f.selector.select(input)).command, bundled);
    assert.equal(f.calls.length, 1);
    assert.equal(f.errors.length, 1);
  }
  const f = fixture([response(invalid)]);
  const selected = await f.selector.select(input);
  assert.equal(selected.key, manifest);
  assert.equal(selected.command, bundled);
  assert.equal(f.calls.length, 1);
});
test("missing_after_install_does_not_loop", async () => {
  const f = fixture([response(missing), response(installed), response(missing)]);
  assert.equal((await f.selector.select(input)).command, bundled);
  assert.equal(f.calls.length, 3);
  assert.equal(f.errors.length, 1);
});
test("concurrent_roots_share_install_and_progress", async () => {
  let installs = 0, progress = 0;
  let finish!: () => void;
  const wait = new Promise<void>((resolve) => {
    finish = resolve;
  });
  let complete = false;
  const selector = new ToolchainSelector({
    async run(command, args) {
      if (args[1] === "install") {
        installs++;
        await wait;
        complete = true;
        return response(installed);
      }
      return response(complete ? compatible : missing);
    },
  }, {
    async progress(title, work) {
      progress++;
      return work();
    },
    async offerInstall() {
      return true;
    },
    errorOnce() {},
    log() {},
  });
  const a = selector.select(input), b = selector.select({ ...input, directory: path.resolve("/work/other") });
  await new Promise((resolve) => setImmediate(resolve));
  finish();
  assert.equal((await a).command, installed);
  assert.equal((await b).command, installed);
  assert.equal(installs, 1);
  assert.equal(progress, 1);
});

test("unknown_schema_logs_a_warning_without_an_error_notification_or_install", async () => {
  const f = fixture([response({ ...compatible, schema: 2 })]);
  const selected = await f.selector.select(input);
  assert.equal(selected.command, bundled);
  assert.equal(f.calls.length, 1);
  assert.equal(f.progress.length, 0);
  assert.equal(f.offers.length, 0);
  assert.equal(f.logs.length, 1);
  assert.deepEqual(f.errors, []);
  assert.match(f.logs[0], /schema/i);
});

test("unknown_schema_after_install_logs_without_an_error_notification", async () => {
  const f = fixture([response(missing), response(installed), response({ ...compatible, schema: 2 })]);
  assert.equal((await f.selector.select(input)).command, bundled);
  assert.equal(f.calls.length, 3);
  assert.equal(f.progress.length, 1);
  assert.equal(f.logs.length, 1);
  assert.deepEqual(f.errors, []);
});


test("install_errors_are_reported_once_for_each_release", async () => {
  const f = fixture(["0.23.0", "0.23.0", "0.24.0"].flatMap((pin) => [
    response({ ...missing, pin }), response("", 2),
  ]));
  for (let i = 0; i < 3; i++) assert.equal((await f.selector.select(input)).command, bundled);
  assert.deepEqual(f.errors, ["install:0.23.0", "install:0.24.0"]);
});
