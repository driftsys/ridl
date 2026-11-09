import assert from "node:assert/strict";
import type { Detection, SignalCheck, SignalCheckEntry } from "../mod.ts";

Deno.test("signal checks accept passing, corrupt and rule results", () => {
  const detection = { rule: "range" } satisfies Detection;
  const checks = [
    (_bytes: Uint8Array) => undefined,
    (_bytes: Uint8Array) => "corrupt",
    (_bytes: Uint8Array) => detection,
  ] satisfies SignalCheck[];
  const bytes = Uint8Array.of(1);
  assert.deepEqual(checks.map((check) => check(bytes)), [
    undefined,
    "corrupt",
    { rule: "range" },
  ]);
  const entry = {
    iface: 1,
    ord: 2,
    check: checks[2],
  } satisfies SignalCheckEntry;
  assert.deepEqual(entry.check(bytes), detection);
});
