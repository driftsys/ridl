import assert from "node:assert/strict";
import * as codec from "./bare.feed.codec.ts";

Deno.test("an interface may take the name of a runtime stream type", () => {
  assert.equal(codec.StreamError.levels.kind, "query");
  assert.equal(codec.StreamSession.levels.kind, "query");
  assert.equal(typeof codec.StreamError_levels, "function");
});
