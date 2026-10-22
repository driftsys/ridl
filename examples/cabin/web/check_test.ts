// Tests of the check in check.ts. Run with `deno test check_test.ts` after
// `just cabin-ws-demo` has written generated/.

import assert from "node:assert/strict";
import * as flatbuffers from "flatbuffers";
import { TemperatureBox } from "./generated/veh/cabin.ts";
import { decodeTemperature, isPass, TemperatureCheck } from "./check.ts";

Deno.test("a full lap passes", () => {
  const check = new TemperatureCheck();
  for (let value = -40; value <= 85; value++) {
    assert.deepEqual(check.accept(value), { kind: "continue" });
  }
  assert.deepEqual(check.accept(-40), { kind: "pass" });
  assert.equal(check.progress.wraps, 1);
  assert.equal(check.progress.received, 127);
});

Deno.test("a value outside the range fails", () => {
  const verdict = new TemperatureCheck().accept(86);
  assert.equal(verdict.kind, "fail");
  assert.match(
    (verdict as { cause: string }).cause,
    /temperature 86 is outside \[-40\.\.85\]/,
  );
});

Deno.test("a value below the range fails", () => {
  const verdict = new TemperatureCheck().accept(-41);
  assert.equal(verdict.kind, "fail");
  assert.match(
    (verdict as { cause: string }).cause,
    /temperature -41 is outside \[-40\.\.85\]/,
  );
  const later = new TemperatureCheck();
  assert.equal(later.accept(22).kind, "continue");
  assert.equal(later.accept(-41).kind, "fail");
});

Deno.test("a step across 85 counts as a wrap", () => {
  const check = new TemperatureCheck();
  check.accept(80);
  assert.deepEqual(check.accept(-30), { kind: "continue" });
  assert.equal(check.progress.wraps, 1);
});

Deno.test("a step of 64 fails", () => {
  const long = new TemperatureCheck();
  long.accept(-40);
  assert.equal(long.accept(24).kind, "fail");
  const short = new TemperatureCheck();
  short.accept(-40);
  assert.equal(short.accept(23).kind, "continue");
});

Deno.test("a repeat is ignored", () => {
  const check = new TemperatureCheck();
  for (const value of [0, 0, 1]) {
    assert.deepEqual(check.accept(value), { kind: "continue" });
  }
  assert.equal(check.progress.received, 3);
});

Deno.test("85 as the first sample does not count", () => {
  const check = new TemperatureCheck();
  assert.deepEqual(check.accept(85), { kind: "continue" });
  assert.deepEqual(check.accept(-40), { kind: "continue" });
});

Deno.test("a fail is sticky", () => {
  const check = new TemperatureCheck();
  const first = check.accept(86);
  assert.equal(first.kind, "fail");
  assert.deepEqual(check.accept(0), first);
  assert.equal(check.progress.received, 1);
});

Deno.test("isPass accepts only a passed progress", () => {
  assert.equal(
    isPass({ received: 1, wraps: 0, sawHighest: false, sawLowestAfter: false }),
    false,
  );
  assert.equal(
    isPass({ received: 5, wraps: 0, sawHighest: true, sawLowestAfter: false }),
    false,
  );
  assert.equal(
    isPass({ received: 0, wraps: 1, sawHighest: true, sawLowestAfter: true }),
    false,
  );
  assert.equal(
    isPass({ received: 5, wraps: 1, sawHighest: false, sawLowestAfter: true }),
    false,
  );
  assert.equal(isPass(undefined), false);
  assert.equal(isPass(3), false);
  const check = new TemperatureCheck();
  for (let value = -40; value <= 85; value++) check.accept(value);
  check.accept(-40);
  assert.equal(isPass(check.progress), true);
});

Deno.test("decodeTemperature reads a TemperatureBox", () => {
  for (const value of [-40, 85]) {
    const builder = new flatbuffers.Builder(16);
    builder.finish(TemperatureBox.createTemperatureBox(builder, value));
    assert.equal(decodeTemperature(builder.asUint8Array()), value);
  }
});
