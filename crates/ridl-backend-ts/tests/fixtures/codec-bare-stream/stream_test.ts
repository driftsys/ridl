import assert from "node:assert/strict";
import type { Caller, Codec, StreamCaller } from "@driftsys/ridl-runtime";
import * as codec from "./bare.feed.codec.ts";

const loaded: Codec = {
  catalogHash: codec.CATALOG_HASH.slice(),
  typeId: () => 1,
  verify: () => ({ ok: true }),
};

Deno.test("an interface may take the name of a runtime stream type", () => {
  assert.equal(codec.StreamError.levels.kind, "query");
  assert.equal(codec.StreamSession.levels.kind, "query");
  assert.equal(typeof codec.StreamErrorClient.bind, "function");
  assert.equal(typeof codec.StreamSessionClient.bind, "function");
});

Deno.test("a stream-only client binds over a stream caller and opens the stream", async () => {
  const calls: { iface: number; ord: number; bound: number }[] = [];
  const port: StreamCaller = {
    catalogHash: codec.CATALOG_HASH.slice(),
    command: () => Promise.reject(new Error("unused")),
    query: () => Promise.reject(new Error("unused")),
    queryStream(iface, ord, _args, opts) {
      calls.push({ iface, ord, bound: opts.bound });
      return ReadableStream.from([]);
    },
    queryDuplex: () => {
      throw new Error("unused");
    },
    commandStream: () => {
      throw new Error("unused");
    },
    queryWithUpload: () => {
      throw new Error("unused");
    },
  };
  const client = codec.StreamErrorClient.bind(port, loaded);
  const levels = client.levels({ bound: 7 });
  assert.deepEqual(await Array.fromAsync(levels), []);
  assert.deepEqual(calls, [{
    iface: codec.StreamError.iface,
    ord: codec.StreamError.levels.ord,
    bound: 7,
  }]);
});

Deno.test("a stream member needs a stream caller, not a caller", () => {
  const callerOnly: Caller = {
    catalogHash: codec.CATALOG_HASH.slice(),
    command: () => Promise.reject(new Error("unused")),
    query: () => Promise.reject(new Error("unused")),
  };
  // Never called: only the type checker reads this line.
  const unused = () => {
    // @ts-expect-error a stream member needs the stream methods of a StreamCaller
    codec.StreamErrorClient.bind(callerOnly, loaded);
  };
  assert.equal(typeof unused, "function");
});
