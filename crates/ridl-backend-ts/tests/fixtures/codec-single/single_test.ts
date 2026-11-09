import assert from "node:assert/strict";
import { decodeReading, encodeReading } from "./single.codec.ts";
import type { Reading } from "./single.ts";

Deno.test("a single-component package encodes and decodes a struct payload", () => {
  const value: Reading = { value: 42 };
  assert.deepEqual(decodeReading(encodeReading(value)), value);
});
