import assert from "node:assert/strict";
import {
  CatalogMismatch,
  CodecError,
  StreamError,
  UnknownPayloadType,
} from "@driftsys/ridl-runtime";
import type {
  Codec,
  PayloadCodec,
  StreamOptions,
  StreamSession,
  StreamTransport,
  VerifyResult,
} from "@driftsys/ridl-runtime";
import * as codec from "./streams.demo.codec.ts";
import type * as t from "./streams.demo.ts";

// Dynamic lookup makes the initial RED an assertion about missing generation.
function generated(name: string) {
  const fn = (codec as unknown as Record<string, unknown>)[name];
  assert.equal(typeof fn, "function", `${name} must be generated`);
  return fn as (
    session: StreamSession,
    ...args: unknown[]
  ) => ReadableStream<unknown>;
}

type Entry = {
  iface: number;
  ord: number;
  kind: string;
  limit?: number;
  request: PayloadCodec<unknown>;
  reply?: PayloadCodec<unknown>;
  streams: { no: number; form: string; element?: PayloadCodec<unknown> }[];
};
function entry(name: string): Entry {
  const feed = (codec as unknown as { Feed: Record<string, unknown> }).Feed;
  assert.ok(feed[name], `${name} needs a stream table entry`);
  return feed[name] as Entry;
}

function source(chunks: Uint8Array[]): ReadableStream<Uint8Array> {
  return new ReadableStream({
    start(controller) {
      for (const chunk of chunks) controller.enqueue(chunk);
      controller.close();
    },
  });
}

async function collect(stream: ReadableStream<unknown>): Promise<unknown[]> {
  const reader = stream.getReader();
  const values = [];
  for (;;) {
    const next = await reader.read();
    if (next.done) return values;
    values.push(next.value);
  }
}

function harness(chunks: Uint8Array[]) {
  const events: string[] = [];
  const calls: {
    iface: number;
    ord: number;
    args: Uint8Array;
    opts: StreamOptions;
  }[] = [];
  const raw = source(chunks);
  const ids = new Map<string, number>();
  const loaded: Codec = {
    catalogHash: codec.CATALOG_HASH.slice(),
    typeId(name) {
      events.push(`resolve:${name}`);
      if (!ids.has(name)) ids.set(name, ids.size + 1);
      return ids.get(name)!;
    },
    verify(id, _bytes) {
      events.push(`verify:${id}`);
      return { ok: true };
    },
  };
  const transport: StreamTransport = {
    queryStream(iface, ord, args, opts) {
      assert.strictEqual(
        this,
        transport,
        "the transport method retains its receiver",
      );
      events.push("request");
      calls.push({ iface, ord, args, opts });
      return raw;
    },
    commandStream() {
      assert.fail("reply streams must call queryStream");
    },
    queryWithUpload() {
      assert.fail("reply streams must call queryStream");
    },
  };
  return { session: { codec: loaded, transport }, calls, events, raw, ids };
}

Deno.test("generated_reply_uses_codec_and_session", async () => {
  const structs = generated("Feed_structs");
  const values: t.Chunk[] = Array.from(
    { length: 64 },
    (_, value) => ({ value: value as t.Count }),
  );
  const h = harness(values.map(codec.encodeChunk));
  const element = entry("structs").streams[0].element!;
  const original = element.decode;
  element.decode = (bytes) => {
    h.events.push("decode");
    return original(bytes);
  };
  try {
    const opts = { bound: 123 };
    assert.deepEqual(
      await collect(structs(h.session, 7 as t.Count, opts)),
      values,
    );
    assert.equal(h.calls.length, 1);
    assert.equal(h.calls[0].iface, 1);
    assert.equal(h.calls[0].ord, 3);
    assert.strictEqual(h.calls[0].opts, opts);
    assert.equal(codec.decodeCount(h.calls[0].args), 7);
    assert.deepEqual(h.events.slice(0, 4), [
      "resolve:streams.demo.Count",
      "resolve:streams.demo.Chunk",
      "verify:1",
      "request",
    ]);
    assert.deepEqual(
      h.events.slice(4),
      values.flatMap(() => ["verify:2", "decode"]),
    );
  } finally {
    element.decode = original;
  }
  const unions = generated("Feed_unions");
  const u = harness([]);
  await collect(
    unions(u.session, 4 as t.Count, 5 as t.Count, 6 as t.Count, { bound: 80 }),
  );
  assert.deepEqual(codec.decodeFeedUnionsParams(u.calls[0].args), {
    session: 4,
    session1: 5,
    opts: 6,
  });
  const scalar = harness([codec.encodeCount(9 as t.Count)]);
  assert.deepEqual(
    await collect(generated("Feed_scalars")(scalar.session, { bound: 50 })),
    [9],
  );
  const raw = harness([new Uint8Array([1, 2])]);
  assert.strictEqual(
    generated("Feed_readBytes")(raw.session, 3 as t.Count, { bound: 50 }),
    raw.raw,
  );
  const text = harness([new TextEncoder().encode("\uFEFFhello 🌍")]);
  assert.deepEqual(
    await collect(generated("Feed_text")(text.session, { bound: 50 })),
    ["\uFEFFhello 🌍"],
  );
});

Deno.test("stream_tables_keep_limits_and_roles", () => {
  generated("Feed_readBytes");
  const bytes = entry("readBytes");
  assert.equal(bytes.limit, 512);
  assert.equal(bytes.request.type, "streams.demo.Count");
  assert.equal(bytes.reply, undefined);
  assert.deepEqual(bytes.streams, [{ no: 0, form: "bytes" }]);
  assert.equal(entry("structs").streams[0].element?.type, "streams.demo.Chunk");
  assert.equal(entry("structs").reply, undefined);
  assert.equal(codec.Feed.latest.reply.type, "streams.demo.Count");
});

Deno.test("setup_errors_send_no_request", () => {
  const fn = generated("Feed_structs");
  for (const byte of [0, 31]) {
    const h = harness([]);
    h.session.codec.catalogHash[byte] ^= 1;
    assert.throws(
      () => fn(h.session, 1 as t.Count, { bound: 1 }),
      CatalogMismatch,
    );
    assert.equal(h.calls.length, 0);
  }
  const h = harness([]);
  h.session.codec.typeId = (name) => {
    if (name === "streams.demo.Chunk") throw new UnknownPayloadType(name);
    return 1;
  };
  assert.throws(
    () => fn(h.session, 1 as t.Count, { bound: 1 }),
    UnknownPayloadType,
  );
  assert.equal(h.calls.length, 0);
  const zero = generated("Feed_zero");
  const row = entry("zero");
  const original = row.limit;
  row.limit = 0;
  try {
    assert.throws(() => zero(h.session, { bound: 1 }), StreamError);
    assert.equal(h.calls.length, 0);
  } finally {
    if (original === undefined) delete row.limit;
    else row.limit = original;
  }
});

Deno.test("request_failures_send_no_request", () => {
  const fn = generated("Feed_structs");
  const results: VerifyResult[] = [
    { ok: false, kind: "malformed", code: 1 },
    { ok: false, kind: "rule", rule: "range", code: 4 },
    { ok: false, kind: "unknown-type", code: -1 },
    { ok: false, kind: "rule", rule: "future", code: 99 },
  ];
  for (const result of results) {
    const h = harness([]);
    h.session.codec.verify = () => result;
    assert.throws(
      () => fn(h.session, 1 as t.Count, { bound: 1 }),
      (error: unknown) => {
        if (result.ok) return false;
        if (result.kind === "malformed") {
          assert.deepEqual((error as StreamError).failure, {
            category: "corrupt",
          });
        } else if (result.kind === "rule" && result.rule === "range") {
          assert.deepEqual((error as StreamError).failure, {
            category: "contract",
            contract: { category: "invalid-value", rule: "range" },
          });
        } else assert.ok(error instanceof CodecError);
        return true;
      },
    );
    assert.equal(h.calls.length, 0);
  }
  const h = harness([]);
  const sentinel = new Error("transport refused");
  h.session.transport.queryStream = () => {
    throw sentinel;
  };
  assert.throws(
    () => fn(h.session, 1 as t.Count, { bound: 1 }),
    (error: unknown) => error === sentinel,
  );
});

// Mutation: access a bare table name inside the generated function body.
Deno.test("stream_table_names_do_not_shadow_captured_entries", async () => {
  for (
    const name of [
      "session",
      "opts",
      "entry",
      "args",
      "raw",
      "element",
      "requestType",
      "elementType",
      "streamEntry",
      "streamEntry1",
    ]
  ) {
    const h = harness([]);
    let result: ReadableStream<unknown> | undefined;
    assert.doesNotThrow(() => {
      result = generated(`${name}_fetch`)(h.session, 7, 8, { bound: 50 });
    }, name);
    if (name !== "element" && name !== "elementType") {
      assert.strictEqual(result, h.raw, name);
    }
    assert.deepEqual(await collect(result!), [], name);
    const table = (codec as unknown as Record<string, { iface: number }>)[name];
    assert.equal(h.calls[0].iface, table.iface, name);
    assert.equal(h.calls[0].ord, 1, name);
  }
});
