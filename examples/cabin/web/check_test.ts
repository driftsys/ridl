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
 * The wasm codec, loaded once for the file. The catalog hash is known only to
 * the server, so the module is first loaded with a placeholder hash, and then
 * again with the hash that the mismatch reports.
 */
const codec: Codec = await (async () => {
  const bytes = await Deno.readFile(
    new URL("./generated/veh_cabin.wasm", import.meta.url),
  );
  try {
    return await loadCodec(bytes, { catalogHash: new Uint8Array(32) });
  } catch (e) {
    if (!(e instanceof CatalogMismatch)) throw e;
    return await loadCodec(bytes, { catalogHash: hashFromHex(e.actual) });
  }
})();

/** The names of a `&[&str]` list in the generated __ridl_wasm.rs. */
const wasmSource = await Deno.readTextFile(
  new URL("../generated/__ridl_wasm.rs", import.meta.url),
);
function wasmList(name: string): string[] {
  const list = wasmSource.match(
    new RegExp(`${name}[^=]*=\\s*&\\[([^\\]]*)\\]`),
  );
  assert.ok(list, `${name} is in __ridl_wasm.rs`);
  return [...list[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
}

Deno.test("TEMPERATURE_TYPE_ID names Temperature in WASM_PAYLOAD_TYPES", () => {
  // decodeTemperature reads a TemperatureBox, the payload of
  // veh.cabin.Temperature.
  assert.equal(
    wasmList("WASM_PAYLOAD_TYPES")[TEMPERATURE_TYPE_ID],
    "veh.cabin.Temperature",
  );
});

Deno.test("RANGE_RULE is 2 plus the index of range in WASM_RULES", () => {
  assert.equal(RANGE_RULE, 2 + wasmList("WASM_RULES").indexOf("range"));
});

Deno.test("the codec accepts 20 and rejects -41 and 86 with the range rule", () => {
  assert.deepEqual(codec.verify(TEMPERATURE_TYPE_ID, encodeTemperature(20)), {
    ok: true,
  });
  for (const value of [-41, 86]) {
    const result = codec.verify(TEMPERATURE_TYPE_ID, encodeTemperature(value));
    assert.equal(result.ok, false);
    assert.equal((result as { kind: string }).kind, "rule");
    assert.equal((result as { code: number }).code, 4);
  }
});

Deno.test("the codec rejects a truncated payload as malformed", () => {
  const result = codec.verify(
    TEMPERATURE_TYPE_ID,
    encodeTemperature(20).slice(0, 4),
  );
  assert.equal(result.ok, false);
  assert.equal((result as { kind: string }).kind, "malformed");
});

Deno.test("selfTest passes with the cabin codec", () => {
  assert.equal(selfTest(codec), undefined);
});

/**
 * A codec whose `verify` gives, for each temperature, the result `judge`
 * returns. It also checks the type id that the caller passes.
 */
function fakeCodec(judge: (value: number) => VerifyResult): Codec {
  return {
    catalogHash: new Uint8Array(32),
    verify(typeId, bytes) {
      assert.equal(typeId, TEMPERATURE_TYPE_ID);
      return judge(decodeTemperature(bytes));
    },
  };
}

const RANGE: VerifyResult = { ok: false, kind: "rule", code: RANGE_RULE };

Deno.test("selfTest passes with a codec that checks the range", () => {
  const good = fakeCodec((v) => (v < -40 || v > 85 ? RANGE : { ok: true }));
  assert.equal(selfTest(good), undefined);
});

Deno.test("selfTest fails a codec that accepts every payload", () => {
  assert.match(selfTest(fakeCodec(() => ({ ok: true }))) ?? "", /accepted -41/);
});

Deno.test("selfTest fails a codec that accepts 86", () => {
  const cause = selfTest(fakeCodec((v) => (v < -40 ? RANGE : { ok: true })));
  assert.match(cause ?? "", /accepted 86/);
});

Deno.test("selfTest fails a codec that rejects 20", () => {
  const cause = selfTest(fakeCodec(() => RANGE));
  assert.match(cause ?? "", /rejected 20\b/);
});

Deno.test("selfTest fails a codec that rejects with another kind or code", () => {
  for (
    const other of [
      { ok: false, kind: "malformed", code: 1 },
      { ok: false, kind: "malformed", code: RANGE_RULE },
      { ok: false, kind: "rule", code: 3 },
    ] as VerifyResult[]
  ) {
    const cause = selfTest(
      fakeCodec((v) => (v < -40 || v > 85 ? other : { ok: true })),
    );
    assert.match(cause ?? "", /-41/);
  }
});

Deno.test("verifyPayload gives the cause of each rejection", () => {
  assert.equal(verifyPayload(codec, encodeTemperature(20)), undefined);
  assert.equal(
    verifyPayload(codec, encodeTemperature(-41)),
    "the codec rejected the payload with rule code 4 (range)",
  );
  assert.equal(
    verifyPayload(codec, encodeTemperature(20).slice(0, 4)),
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
