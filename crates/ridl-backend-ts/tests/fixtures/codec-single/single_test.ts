import assert from "node:assert/strict";
import type { Codec } from "@driftsys/ridl-runtime";
import { FakePort } from "@ridl-runtime-test/tests/fake_port.ts";
import {
  CATALOG_HASH,
  decodeReading,
  encodeReading,
  SensorClient,
} from "./single.codec.ts";
import { SensorClient as UntimedSensorClient } from "./single_untimed.codec.ts";
import type { Reading } from "./single.ts";

Deno.test("a single-component package encodes and decodes a struct payload", () => {
  const value: Reading = { value: 42 };
  assert.deepEqual(decodeReading(encodeReading(value)), value);
});

Deno.test("a command defaults its bound to the declared maximum", async () => {
  const port = new FakePort(CATALOG_HASH.slice());
  const loaded: Codec = {
    catalogHash: CATALOG_HASH.slice(),
    typeId: () => 1,
    verify: () => ({ ok: true }),
  };
  const client = SensorClient.bind(port, loaded);
  await client.report({ value: 42 } as Reading);
  await client.report({ value: 42 } as Reading, { bound: 9 });
  assert.deepEqual(
    port.calls.map((call) => (call.opts as { bound: number }).bound),
    [50, 9],
  );
});

Deno.test("a member with no declared maximum requires a bound", async () => {
  const port = new FakePort(CATALOG_HASH.slice());
  const loaded: Codec = {
    catalogHash: CATALOG_HASH.slice(),
    typeId: () => 1,
    verify: () => ({ ok: true }),
  };
  const client = UntimedSensorClient.bind(port, loaded);
  const reading = { value: 42 } as Reading;
  // Never called: only the type checker reads these lines.
  const unused = () => {
    // @ts-expect-error `opts` is required when no maximum is declared
    client.report(reading);
    // @ts-expect-error `opts.bound` is required when no maximum is declared
    client.report(reading, {});
  };
  assert.equal(typeof unused, "function");
  await client.report(reading, { bound: 5 });
  assert.deepEqual(
    port.calls.map((call) => (call.opts as { bound: number }).bound),
    [5],
  );
});
