// Tests of the check in check.ts. Run with `deno test --allow-read
// check_test.ts` after `just cabin-ws-demo` has written generated/, which
// holds the `flatc` code and the wasm codec veh_cabin.wasm.

import assert from "node:assert/strict";
import * as flatbuffers from "flatbuffers";
import { TemperatureBox } from "./generated/veh/cabin.ts";
import {
  CatalogMismatch,
  type Codec,
  loadCodec,
  type VerifyResult,
} from "../../../packages/ridl-runtime/mod.ts";
import {
  decodeTemperature,
  encodeTemperature,
  hashFromHex,
  isPass,
  RANGE_RULE,
  selfTest,
  TEMPERATURE_TYPE_ID,
  TemperatureCheck,
  verifyPayload,
} from "./check.ts";

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

/**
 * The wasm codec. The catalog hash is known only to the server, so the module
 * is first loaded with a placeholder hash, and then again with the hash that
 * the mismatch reports.
 */
async function codec(): Promise<Codec> {
  const bytes = await Deno.readFile(
    new URL("./generated/veh_cabin.wasm", import.meta.url),
  );
  try {
    return await loadCodec(bytes, { catalogHash: new Uint8Array(32) });
  } catch (e) {
    if (!(e instanceof CatalogMismatch)) throw e;
    return await loadCodec(bytes, { catalogHash: hashFromHex(e.actual) });
  }
}

Deno.test("TEMPERATURE_TYPE_ID names Temperature in WASM_PAYLOAD_TYPES", async () => {
  const source = await Deno.readTextFile(
    new URL("../generated/__ridl_wasm.rs", import.meta.url),
  );
  const list = source.match(/WASM_PAYLOAD_TYPES[^=]*=\s*&\[([^\]]*)\]/);
  assert.ok(list, "WASM_PAYLOAD_TYPES is in __ridl_wasm.rs");
  const names = [...list[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
  // decodeTemperature reads a TemperatureBox, the payload of
  // veh.cabin.Temperature.
  assert.equal(names[TEMPERATURE_TYPE_ID], "veh.cabin.Temperature");
});

Deno.test("the codec accepts 20 and rejects -41 and 86 with the range rule", async () => {
  const c = await codec();
  assert.equal(RANGE_RULE, 4);
  assert.deepEqual(c.verify(TEMPERATURE_TYPE_ID, encodeTemperature(20)), {
    ok: true,
  });
  for (const value of [-41, 86]) {
    const result = c.verify(TEMPERATURE_TYPE_ID, encodeTemperature(value));
    assert.equal(result.ok, false);
    assert.equal((result as { kind: string }).kind, "rule");
    assert.equal((result as { code: number }).code, 4);
  }
});

Deno.test("the codec rejects a truncated payload as malformed", async () => {
  const c = await codec();
  const result = c.verify(
    TEMPERATURE_TYPE_ID,
    encodeTemperature(20).slice(0, 4),
  );
  assert.equal(result.ok, false);
  assert.equal((result as { kind: string }).kind, "malformed");
});

Deno.test("selfTest passes with the cabin codec", async () => {
  assert.equal(selfTest(await codec()), undefined);
});

Deno.test("verifyPayload gives the cause of each rejection", async () => {
  const c = await codec();
  assert.equal(verifyPayload(c, encodeTemperature(20)), undefined);
  assert.equal(
    verifyPayload(c, encodeTemperature(-41)),
    "the codec rejected the payload with rule code 4 (range)",
  );
  assert.equal(
    verifyPayload(c, encodeTemperature(20).slice(0, 4)),
    "the codec found the payload malformed (code 1)",
  );
  // A codec that reports the other kinds, to check the remaining strings.
  const fake = (result: VerifyResult) =>
    ({ catalogHash: new Uint8Array(32), verify: () => result }) as Codec;
  assert.equal(
    verifyPayload(fake({ ok: false, kind: "rule", code: 3 }), new Uint8Array()),
    "the codec rejected the payload with rule code 3",
  );
  assert.equal(
    verifyPayload(
      fake({ ok: false, kind: "unknown-type", code: -1 }),
      new Uint8Array(),
    ),
    `the codec does not know type id ${TEMPERATURE_TYPE_ID}`,
  );
});

Deno.test("encodeTemperature round-trips through decodeTemperature", () => {
  for (const value of [-41, 20, 86]) {
    assert.equal(decodeTemperature(encodeTemperature(value)), value);
  }
});
