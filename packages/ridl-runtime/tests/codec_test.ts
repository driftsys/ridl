import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert";
import { CatalogMismatch, CodecError, loadCodec } from "../mod.ts";
import { fixtureBytes, HASH } from "./fixture.ts";

const hex = (b: Uint8Array) =>
  Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");

const options = { catalogHash: HASH };

// Loads the fixture and returns the module instance beside the codec, so a
// test can read the counters that the module exports.
async function counted() {
  let instance!: WebAssembly.Instance;
  const realInstantiate = WebAssembly.instantiate;
  // loadCodec calls WebAssembly.instantiate with bytes; record the instance.
  (WebAssembly as { instantiate: unknown }).instantiate = async (
    input: BufferSource,
    imports?: WebAssembly.Imports,
  ) => {
    const result = await realInstantiate(input as BufferSource, imports);
    instance = (result as WebAssembly.WebAssemblyInstantiatedSource).instance;
    return result;
  };
  try {
    const codec = await loadCodec(fixtureBytes(), options);
    return { codec, instance };
  } finally {
    (WebAssembly as { instantiate: unknown }).instantiate = realInstantiate;
  }
}

const global = (i: WebAssembly.Instance, name: string) =>
  (i.exports[name] as WebAssembly.Global).value as number;

Deno.test("loads from bytes and reports the catalog hash", async () => {
  const codec = await loadCodec(fixtureBytes(), options);
  assertEquals(codec.catalogHash, HASH);
});

Deno.test("loads from a Response with the wasm content type", async () => {
  const response = new Response(fixtureBytes(), {
    headers: { "content-type": "application/wasm" },
  });
  const codec = await loadCodec(response, options);
  assertEquals(codec.verify(1, new Uint8Array([9])), { ok: true });
});

Deno.test("loads from a Response without the wasm content type", async () => {
  const codec = await loadCodec(
    Promise.resolve(new Response(fixtureBytes())),
    options,
  );
  assertEquals(codec.catalogHash, HASH);
});

Deno.test("rejects a Response with an error status", async () => {
  await assertRejects(
    () => loadCodec(new Response("no", { status: 404 }), options),
    CodecError,
    "404",
  );
});

Deno.test("loads from a file: URL", async () => {
  const dir = await Deno.makeTempDir();
  try {
    await Deno.writeFile(`${dir}/codec.wasm`, fixtureBytes());
    const codec = await loadCodec(
      new URL(`file://${dir}/codec.wasm`),
      options,
    );
    assertEquals(codec.catalogHash, HASH);
  } finally {
    await Deno.remove(dir, { recursive: true });
  }
});

Deno.test("loads from an http: URL with fetch", async () => {
  const bytes = fixtureBytes();
  const server = Deno.serve(
    { port: 0, onListen() {} },
    () =>
      new Response(bytes, { headers: { "content-type": "application/wasm" } }),
  );
  try {
    const url = new URL(`http://127.0.0.1:${server.addr.port}/codec.wasm`);
    const codec = await loadCodec(url, options);
    assertEquals(codec.catalogHash, HASH);
  } finally {
    await server.shutdown();
  }
});

Deno.test("throws CatalogMismatch with both hashes in the message", async () => {
  const wrong = HASH.slice();
  wrong[0] = 0xff;
  const error = await assertRejects(
    () => loadCodec(fixtureBytes(), { catalogHash: wrong }),
    CatalogMismatch,
  );
  assertEquals(error.expected, hex(wrong));
  assertEquals(error.actual, hex(HASH));
  assert(error.message.includes(error.expected));
  assert(error.message.includes(error.actual));
});

Deno.test("throws CodecError naming a missing export", async () => {
  const error = await assertRejects(
    () => loadCodec(fixtureBytes(false), options),
    CodecError,
    "ridl_verify",
  );
  assertEquals(error.exportName, "ridl_verify");
});

Deno.test("throws CodecError for bytes that are not wasm", async () => {
  await assertRejects(
    () => loadCodec(new Uint8Array([1, 2, 3]), options),
    CodecError,
  );
});

Deno.test("throws CodecError for a catalog hash of the wrong length", async () => {
  await assertRejects(
    () => loadCodec(fixtureBytes(), { catalogHash: new Uint8Array(3) }),
    CodecError,
    "32",
  );
});

Deno.test("verify maps ok, malformed, rule and unknown type", async () => {
  const text = ["rule two", "rule three"];
  const codec = await loadCodec(fixtureBytes(), {
    ...options,
    ruleText: text,
  });
  assertEquals(codec.verify(0, new Uint8Array([0])), { ok: true });
  assertEquals(codec.verify(0, new Uint8Array([1])), {
    ok: false,
    kind: "malformed",
    code: 1,
  });
  assertEquals(codec.verify(0, new Uint8Array([2])), {
    ok: false,
    kind: "rule",
    code: 2,
    rule: "rule two",
  });
  assertEquals(codec.verify(0, new Uint8Array([3])), {
    ok: false,
    kind: "rule",
    code: 3,
    rule: "rule three",
  });
  assertEquals(codec.verify(5, new Uint8Array([0])), {
    ok: false,
    kind: "unknown-type",
    code: -1,
  });
});

Deno.test("a rule code without text has no rule field", async () => {
  const codec = await loadCodec(fixtureBytes(), options);
  assertEquals(codec.verify(0, new Uint8Array([4])), {
    ok: false,
    kind: "rule",
    code: 4,
  });
});

Deno.test("verify accepts empty bytes", async () => {
  const codec = await loadCodec(fixtureBytes(), options);
  assertEquals(codec.verify(0, new Uint8Array(0)), {
    ok: false,
    kind: "malformed",
    code: 1,
  });
});

Deno.test("verify works after the memory grows", async () => {
  const { codec, instance } = await counted();
  const memory = instance.exports.memory as WebAssembly.Memory;
  const before = memory.buffer.byteLength;
  // A payload larger than the memory forces ridl_alloc to grow it, which
  // detaches the old buffer.
  const big = new Uint8Array(before + 10);
  big[0] = 2;
  assertEquals(codec.verify(0, big), {
    ok: false,
    kind: "rule",
    code: 2,
  });
  assert(memory.buffer.byteLength > before, "the memory did not grow");
  assertEquals(codec.verify(0, new Uint8Array([0])), { ok: true });
});

Deno.test("verify frees on every path", async () => {
  const { codec, instance } = await counted();
  // The load allocates and frees the hash buffer once.
  const base = global(instance, "frees");
  assertEquals(global(instance, "allocs"), base);
  for (const [t, b] of [[0, 0], [0, 1], [0, 2], [7, 0]]) {
    codec.verify(t, new Uint8Array([b]));
  }
  codec.verify(0, new Uint8Array(0));
  assertEquals(global(instance, "frees"), base + 5);
  assertEquals(global(instance, "allocs"), base + 5);
});

Deno.test("verify frees when the wasm call throws", async () => {
  const { codec, instance } = await counted();
  const base = global(instance, "frees");
  assertThrows(() =>
    codec.verify(1n as unknown as number, new Uint8Array([1]))
  );
  assertEquals(global(instance, "frees"), base + 1);
});
