// The module of a package whose only payload root is a `ridl.std` scalar:
// the interface table encodes through `ridl.std.codec.ts`. The recipe
// `_codec-ts-check` copies this file beside the generated files.

import assert from "node:assert/strict";
import * as codec from "./veh.names.codec.ts";
import type * as ridl_std from "./ridl.std.ts";

Deno.test("the table of a foreign-only package encodes through ridl.std's codec", () => {
  const value = "ann" as ridl_std.Name;
  assert.equal(codec.Names.rename.request.type, "ridl.std.Name");
  assert.equal(
    codec.Names.rename.request.decode(codec.Names.rename.request.encode(value)),
    value,
  );
  assert.deepEqual(codec.PAYLOAD_TYPES, []);
});
