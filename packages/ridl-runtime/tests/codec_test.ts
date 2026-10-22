import { assert, assertEquals, assertRejects, assertThrows } from "@std/assert";
import { CatalogMismatch, CodecError, loadCodec } from "../mod.ts";
import { fixtureBytes, type FixtureOptions, HASH } from "./fixture.ts";

const hex = (b: Uint8Array) =>
  Array.from(b, (x) => x.toString(16).padStart(2, "0")).join("");

const options = { catalogHash: HASH };

type Raw = { instantiate: unknown; instantiateStreaming: unknown };

// Runs `run` while WebAssembly.instantiate records the instance it creates, so
// a test can read the counters that the module exports.
async function withInstance<T>(
  run: () => Promise<T>,
): Promise<
  { result: T | undefined; error?: unknown; instance: WebAssembly.Instance }
> {
  let instance!: WebAssembly.Instance;
  const realInstantiate = WebAssembly.instantiate;
  (WebAssembly as unknown as Raw).instantiate = async (
    input: BufferSource,
    imports?: WebAssembly.Imports,
  ) => {
    const result = await realInstantiate(input, imports);
    instance = (result as WebAssembly.WebAssemblyInstantiatedSource).instance;
    return result;
  };
  try {
    return { result: await run(), instance };
  } catch (error) {
    return { result: undefined, error, instance };
  } finally {
    (WebAssembly as unknown as Raw).instantiate = realInstantiate;
  }
}

async function counted(
  opts = options,
  fixture: FixtureOptions = {},
) {
  const { result, error, instance } = await withInstance(() =>
    loadCodec(fixtureBytes(fixture), opts)
  );
  if (error) throw error;
  return { codec: result!, instance };
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
    () => loadCodec(fixtureBytes({ verify: false }), options),
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

Deno.test("verify frees when the wasm call traps", async () => {
  const { codec, instance } = await counted(options, { trap: true });
  const base = global(instance, "frees");
  assertThrows(
    () => codec.verify(1, new Uint8Array([1])),
    WebAssembly.RuntimeError,
  );
  assertEquals(global(instance, "frees"), base + 1);
});

Deno.test("streaming is used only for the application/wasm content type", async () => {
  const realInstantiate = WebAssembly.instantiate;
  const realStreaming = WebAssembly.instantiateStreaming;
  const cases: [string, "streaming" | "bytes"][] = [
    ["application/wasm", "streaming"],
    ["Application/WASM; charset=x", "streaming"],
    ["text/plain", "bytes"],
    ["application/octet-stream", "bytes"],
    ["application/x-wasm", "bytes"],
  ];
  try {
    for (const [type, expected] of cases) {
      const used: string[] = [];
      (WebAssembly as unknown as Raw).instantiate = (
        input: BufferSource,
        imports?: WebAssembly.Imports,
      ) => {
        used.push("bytes");
        return realInstantiate(input, imports);
      };
      (WebAssembly as unknown as Raw).instantiateStreaming = async (
        response: Response,
        imports?: WebAssembly.Imports,
      ) => {
        used.push("streaming");
        return await realInstantiate(await response.arrayBuffer(), imports);
      };
      const response = new Response(fixtureBytes(), {
        headers: { "content-type": type },
      });
      await loadCodec(response, options);
      assertEquals(used, [expected], type);
    }
  } finally {
    (WebAssembly as unknown as Raw).instantiate = realInstantiate;
    (WebAssembly as unknown as Raw).instantiateStreaming = realStreaming;
  }
});

Deno.test("verify works after two consecutive growths", async () => {
  const { codec, instance } = await counted();
  const memory = instance.exports.memory as WebAssembly.Memory;
  const first = memory.buffer.byteLength;
  const big = new Uint8Array(first + 10);
  big[0] = 2;
  assertEquals(codec.verify(0, big).ok, false);
  const second = memory.buffer.byteLength;
  assert(second > first);
  const bigger = new Uint8Array(second + 10);
  bigger[0] = 3;
  assertEquals(codec.verify(0, bigger), { ok: false, kind: "rule", code: 3 });
  assert(memory.buffer.byteLength > second, "the memory did not grow twice");
});

Deno.test("catalogHash stays valid after the memory grows", async () => {
  const { codec, instance } = await counted();
  const memory = instance.exports.memory as WebAssembly.Memory;
  const hash = codec.catalogHash;
  codec.verify(0, new Uint8Array(memory.buffer.byteLength + 10));
  assertEquals(hash.length, 32);
  assertEquals(hash, HASH);
  assertEquals(codec.catalogHash, HASH);
});

Deno.test("a catalog mismatch frees the buffer it allocated", async () => {
  const wrong = HASH.slice();
  wrong[31] ^= 1;
  const { error, instance } = await withInstance(() =>
    loadCodec(fixtureBytes(), { catalogHash: wrong })
  );
  assert(error instanceof CatalogMismatch);
  assertEquals(global(instance, "allocs"), 1);
  assertEquals(global(instance, "frees"), 1);
  assertEquals(global(instance, "freePtr"), global(instance, "allocPtr"));
  assertEquals(global(instance, "freeLen"), 32);
});

Deno.test("verify frees the buffer it allocated, after the call", async () => {
  const { codec, instance } = await counted();
  codec.verify(0, new Uint8Array([0, 0, 0, 0, 0]));
  assertEquals(global(instance, "freePtr"), global(instance, "allocPtr"));
  assertEquals(global(instance, "freeLen"), 5);
  assertEquals(global(instance, "allocLen"), 5);
  assert(global(instance, "freeSeq") > global(instance, "verifySeq"));
});

Deno.test("a catalog hash of 33 or 0 bytes is a CodecError", async () => {
  for (const length of [33, 0]) {
    await assertRejects(
      () => loadCodec(fixtureBytes(), { catalogHash: new Uint8Array(length) }),
      CodecError,
      "32",
    );
  }
});

Deno.test("a module without memory is a CodecError naming memory", async () => {
  const error = await assertRejects(
    () => loadCodec(fixtureBytes({ memory: false }), options),
    CodecError,
  );
  assertEquals(error.exportName, "memory");
});

Deno.test("an export that is not a function is a CodecError", async () => {
  const error = await assertRejects(
    () => loadCodec(fixtureBytes({ freeIsGlobal: true }), options),
    CodecError,
    "ridl_free",
  );
  assertEquals(error.exportName, "ridl_free");
});

Deno.test("a result code below -1 is a CodecError", async () => {
  const codec = await loadCodec(fixtureBytes({ badCode: true }), options);
  assertThrows(() => codec.verify(1, new Uint8Array([0])), CodecError, "-5");
});

Deno.test("an invalid typeId is a CodecError and allocates nothing", async () => {
  const { codec, instance } = await counted();
  const allocs = global(instance, "allocs");
  for (const id of [-1, 1.5, 2 ** 32, NaN]) {
    assertThrows(
      () => codec.verify(id, new Uint8Array(1)),
      CodecError,
      "typeId",
    );
  }
  assertEquals(global(instance, "allocs"), allocs);
  assertEquals(codec.verify(2 ** 32 - 1, new Uint8Array(1)).ok, false);
});

Deno.test("loadCodec rejects when the hash buffer cannot be allocated", async () => {
  const error = await assertRejects(
    () => loadCodec(fixtureBytes({ allocFails: "always" }), options),
    CodecError,
    "out of memory",
  );
  assertEquals(error.exportName, "ridl_alloc");
});

Deno.test("verify throws when the payload buffer cannot be allocated", async () => {
  const codec = await loadCodec(fixtureBytes({ allocFails: 32 }), options);
  const error = assertThrows(
    () => codec.verify(0, new Uint8Array(64)),
    CodecError,
    "out of memory",
  );
  assertEquals(error.exportName, "ridl_alloc");
});

Deno.test("verify of an empty payload does not throw", async () => {
  const codec = await loadCodec(
    fixtureBytes({ allocNullForEmpty: true }),
    options,
  );
  assertEquals(codec.verify(0, new Uint8Array(0)).ok, false);
});
