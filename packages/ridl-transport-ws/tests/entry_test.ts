import assert from "node:assert/strict";
import * as entry from "../mod.ts";

Deno.test("the package entry point exports BLOCK_BYTES and keeps the rest of blocks.ts internal", () => {
  assert.equal(entry.BLOCK_BYTES, 4_096);
  const keys = Object.keys(entry);
  assert.ok(!keys.includes("FORM_BLOCK"));
  assert.ok(!keys.includes("blockCount"));
});
