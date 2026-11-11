import assert from "node:assert/strict";
import * as codec from "./bare.gauge.codec.ts";

Deno.test("a package with a signal and no string uses the runtime error", () => {
  assert.equal(codec.Gauge.level.kind, "signal");
  assert.equal(typeof codec.signalChecks, "function");
});
