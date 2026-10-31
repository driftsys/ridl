import assert from "node:assert/strict";
import { hex } from "./block_vectors.ts";

Deno.test("hex ignores spaces and backticks and reads whole bytes", () => {
  assert.deepEqual(hex("0a `ff` 01"), Uint8Array.from([0x0a, 0xff, 0x01]));
});

Deno.test("hex throws on an odd number of digits", () => {
  assert.throws(() => hex("0a f"), /odd hex digits/);
});
