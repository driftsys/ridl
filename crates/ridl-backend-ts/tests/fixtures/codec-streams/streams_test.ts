import {
  connect,
  decode,
  Kind,
  SessionError,
  SUBPROTOCOL_V3,
} from "@ridl-test/mod.ts";
import type { Frame, Target } from "@ridl-test/mod.ts";
import { FakeSocket } from "@ridl-test/tests/fake_socket.ts";
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

// Mutation: bypass request verification in raw byte or text functions.
for (const name of ["Feed_readBytes", "Feed_text"]) {
  Deno.test(`raw_request_failures_send_no_request_${name}`, () => {
    const h = harness([]);
    h.session.codec.verify = () => ({ ok: false, kind: "malformed", code: 1 });
    const fn = generated(name);
    const args = name === "Feed_readBytes" ? [1, { bound: 1 }] : [{ bound: 1 }];
    assert.throws(() => fn(h.session, ...args), StreamError);
    assert.equal(h.calls.length, 0);
    assert.ok(!h.events.includes("request"));
  });
}

type UploadCall = {
  upload: WritableStream<unknown>;
  accepted?: Promise<void>;
  reply?: Promise<unknown>;
};
function uploadFunction(name: string) {
  const fn = (codec as unknown as Record<string, unknown>)[`Feed_${name}`];
  assert.equal(typeof fn, "function", `Feed_${name} must be generated`);
  return fn as (session: StreamSession, ...args: unknown[]) => UploadCall;
}
const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function uploadHarness() {
  const h = harness([]);
  const chunks: Uint8Array[] = [];
  const raw = new WritableStream<Uint8Array>({
    write(bytes) {
      chunks.push(bytes.slice());
    },
  });
  const accepted = Promise.resolve();
  const reply = Promise.withResolvers<Uint8Array>();
  let method = "";
  for (const key of ["commandStream", "queryWithUpload"] as const) {
    const call = function (
      this: StreamTransport,
      iface: number,
      ord: number,
      args: Uint8Array,
      opts: StreamOptions,
    ) {
      assert.strictEqual(this, h.session.transport);
      method = key;
      h.events.push("request");
      h.calls.push({ iface, ord, args, opts });
      return { upload: raw, accepted, reply: reply.promise };
    };
    h.session.transport[key] = call;
  }
  return {
    ...h,
    chunks,
    raw,
    accepted,
    reply,
    get method() {
      return method;
    },
  };
}

Deno.test("generated_upload_encodes_only_plain_arguments", async () => {
  for (
    const name of [
      "uploadNamed",
      "uploadChoice",
      "uploadText",
      "uploadData",
      "upload",
    ]
  ) {
    const fn = uploadFunction(name);
    const h = uploadHarness();
    const opts = { bound: 94 };
    const args = name === "uploadNamed" || name === "uploadChoice"
      ? [7, 8, opts]
      : name === "uploadText"
      ? [7, opts]
      : [opts];
    let call!: UploadCall;
    assert.doesNotThrow(() => {
      call = fn(h.session, ...args);
    });
    assert.ok(call.upload instanceof WritableStream);
    const row = entry(name);
    assert.deepEqual(
      row.request.decode(h.calls[0].args),
      args.length === 3 ? { seed: 7, tail: 8 } : args.length === 2 ? 7 : {},
    );
    assert.equal(h.calls[0].iface, row.iface);
    assert.equal(h.calls[0].ord, row.ord);
    assert.strictEqual(h.calls[0].opts, opts);
    assert.equal(
      h.method,
      name === "uploadChoice" ? "queryWithUpload" : "commandStream",
    );
    assert.equal(row.streams[0].no, 1);
    if (name === "uploadChoice") {
      h.reply.resolve(
        entry(name).reply!.encode({ kind: "ok", value: { value: 9 } }),
      );
      assert.deepEqual(await call.reply, { kind: "ok", value: { value: 9 } });
    } else assert.strictEqual(call.accepted, h.accepted);
    const writer = call.upload.getWriter();
    const value = name === "uploadText"
      ? "abc😀"
      : name === "uploadData" || name === "upload"
      ? Uint8Array.of(1, 2)
      : { value: 5 };
    await writer.write(value);
    await writer.close();
    assert.equal(h.raw.locked, false);
    if (row.streams[0].element) {
      assert.deepEqual(row.streams[0].element.decode(h.chunks[0]), value);
    }
    assert.ok(h.events.indexOf("verify:1") < h.events.indexOf("request"));
    // A zero limit, catalog mismatch or unknown type cannot open a request.
    const limit = row.limit;
    row.limit = 0;
    try {
      const zero = uploadHarness();
      assert.throws(() => fn(zero.session, ...args), StreamError);
      assert.equal(zero.calls.length, 0);
    } finally {
      if (limit === undefined) delete row.limit;
      else row.limit = limit;
    }
    const invalid = uploadHarness();
    invalid.session.codec.verify = () => ({
      ok: false,
      kind: "malformed",
      code: 1,
    });
    assert.throws(() => fn(invalid.session, ...args), StreamError);
    assert.equal(invalid.calls.length, 0);
    const mismatch = uploadHarness();
    mismatch.session.codec.catalogHash[0] ^= 1;
    assert.throws(() => fn(mismatch.session, ...args), CatalogMismatch);
    assert.equal(mismatch.calls.length, 0);
    for (
      const type of [
        row.request.type,
        row.streams[0].element?.type,
        row.reply?.type,
      ]
    ) {
      if (type === undefined) continue;
      const unknown = uploadHarness();
      unknown.session.codec.typeId = (name) => {
        if (name === type) throw new UnknownPayloadType(name);
        return 1;
      };
      assert.throws(() => fn(unknown.session, ...args), UnknownPayloadType);
      assert.equal(unknown.calls.length, 0);
    }
    if (row.streams[0].element) {
      const invalidElement = uploadHarness();
      const upload = fn(invalidElement.session, ...args);
      // Keep a query reply observed while the rejected write aborts the upload.
      upload.reply?.catch(() => {});
      invalidElement.session.codec.verify = () => ({
        ok: false,
        kind: "rule",
        rule: "range",
        code: 4,
      });
      const writer = upload.upload.getWriter();
      writer.closed.catch(() => {});
      await assert.rejects(writer.write(value), StreamError);
      await assert.rejects(writer.write(value), StreamError);
      assert.equal(invalidElement.chunks.length, 0);
      assert.equal(invalidElement.raw.locked, false);
    }
  }
});

Deno.test("plain_reply_is_verified_before_decode", async () => {
  for (
    const [name, value] of [["uploadChoice", {
      kind: "err",
      value: { kind: "unavailable", value: {} },
    }], [
      "uploadTuple",
      { first: 1, tail: 2 },
    ]] as const
  ) {
    const fn = uploadFunction(name);
    const payload = entry(name).reply!;
    for (
      const result of [
        { ok: true },
        { ok: false, kind: "malformed", code: 1 },
        { ok: false, kind: "rule", rule: "range", code: 4 },
      ] as VerifyResult[]
    ) {
      const h = uploadHarness();
      const args = name === "uploadChoice"
        ? [7, 8, { bound: 99 }]
        : [{ bound: 99 }];
      const call = fn(h.session, ...args);
      let decodes = 0;
      const original = payload.decode;
      payload.decode = (bytes) => {
        decodes++;
        return original(bytes);
      };
      h.session.codec.verify = () => result;
      try {
        h.reply.resolve(payload.encode(value));
        if (result.ok) {
          assert.deepEqual(await call.reply, value);
          assert.equal(decodes, 1);
        } else {
          await assert.rejects(call.reply!, StreamError);
          assert.equal(decodes, 0);
        }
      } finally {
        payload.decode = original;
        await call.upload.abort();
      }
    }
  }
});

function frames(socket: FakeSocket): Frame[] {
  return socket.sent.map((bytes) => {
    const frame = decode(bytes);
    assert.ok(frame.ok);
    return frame.frame;
  });
}
async function socketHarness() {
  let socket!: FakeSocket;
  const pending = connect("ws://test", {
    catalog: { name: codec.CATALOG_NAME, hash: codec.CATALOG_HASH },
    interfaces: [codec.Feed],
    webSocket: (url, protocols) => {
      socket = new FakeSocket(url, protocols);
      return socket as unknown as WebSocket;
    },
  });
  socket.open(SUBPROTOCOL_V3);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  const transport = await pending;
  const session = { transport, codec: harness([]).session.codec };
  return { socket, session };
}
function target(name: string): Target {
  const row = entry(name);
  return {
    kind: row.kind === "command" ? Kind.Command : Kind.Query,
    iface: row.iface,
    ord: row.ord,
  };
}
function observe<T>(promise: Promise<T>) {
  const state: { settled: boolean; value?: T; error?: unknown } = {
    settled: false,
  };
  promise.then((value) => {
    state.settled = true;
    state.value = value;
  }, (error) => {
    state.settled = true;
    state.error = error;
  });
  return state;
}
async function spend(writer: WritableStreamDefaultWriter<unknown>) {
  for (let i = 0; i < 4; i++) await writer.write({ value: i });
  const fifth = writer.write({ value: 4 });
  const state = observe(fifth);
  await tick();
  assert.equal(state.settled, false, "write five waits for raw credit");
  return { fifth, state };
}
Deno.test("typed_write_waits_for_credit_and_close_FakeSocket", async () => {
  const fn = uploadFunction("uploadNamed");
  const { session, socket } = await socketHarness();
  try {
    const call = fn(session, 1, 2, { bound: 1000 });
    const writer = call.upload.getWriter();
    socket.deliver({
      form: "response",
      envelope: { stamp: 0n, seq: 1n },
      target: target("uploadNamed"),
      correlation: 1n,
      outcome: { outcome: "accepted" },
    });
    await call.accepted;
    const { fifth, state } = await spend(writer);
    const close = writer.close();
    await tick();
    assert.equal(frames(socket).filter((f) => f.form === "chunk").length, 4);
    assert.equal(
      frames(socket).filter((f) => f.form === "stream_end").length,
      0,
    );
    socket.deliver({
      form: "stream_credit",
      target: target("uploadNamed"),
      correlation: 1n,
      stream: 1,
      credit: 1,
    });
    await fifth;
    await close;
    assert.equal(state.settled, true);
    const chunks = frames(socket).filter((f) => f.form === "chunk");
    assert.deepEqual(
      chunks.map((f) =>
        entry("uploadNamed").streams[0].element!.decode(f.payload)
      ),
      [0, 1, 2, 3, 4].map((value) => ({ value })),
    );
    const ends = frames(socket).filter((f) => f.form === "stream_end");
    assert.deepEqual(
      ends.map((f) => [f.stream, f.envelope.seq, f.outcome.kind]),
      [[1, 6n, "end"]],
    );
  } finally {
    session.transport.close();
  }
});
Deno.test("remote_refusal_errors_typed_writer_FakeSocket", async () => {
  const fn = uploadFunction("uploadNamed");
  for (const rule of ["range", "utf8"] as const) {
    for (const queuedClose of [false, true]) {
      const { session, socket } = await socketHarness();
      try {
        const call = fn(session, 1, 2, { bound: 1000 });
        const writer = call.upload.getWriter();
        const closed = writer.closed.catch((e) => e);
        socket.deliver({
          form: "response",
          envelope: { stamp: 0n, seq: 1n },
          target: target("uploadNamed"),
          correlation: 1n,
          outcome: { outcome: "accepted" },
        });
        await call.accepted;
        const { fifth } = await spend(writer);
        const failed = fifth.catch((e) => e);
        const close = queuedClose ? writer.close().catch((e) => e) : undefined;
        socket.deliver({
          form: "stream_end",
          target: target("uploadNamed"),
          correlation: 1n,
          stream: 1,
          envelope: { stamp: 0n, seq: 1n },
          outcome: {
            kind: "contract",
            contract: { category: "invalid-value", rule },
          },
        });
        const remote = await failed;
        assert.ok(remote instanceof SessionError);
        assert.deepEqual(remote.failure, {
          category: "contract",
          contract: { category: "invalid-value", rule },
        });
        assert.strictEqual(
          await writer.write({ value: 9 }).catch((e) => e),
          remote,
        );
        if (queuedClose) assert.strictEqual(await close, remote);
        else await assert.rejects(writer.close(), TypeError);
        assert.strictEqual(await closed, remote);
        assert.equal(await call.accepted, undefined);
        assert.equal(
          frames(socket).filter((f) => f.form === "chunk").length,
          4,
        );
      } finally {
        session.transport.close();
      }
    }
  }
});
Deno.test("abort_releases_a_credit_waiter_FakeSocket", async () => {
  const fn = uploadFunction("uploadNamed");
  const { session, socket } = await socketHarness();
  try {
    const call = fn(session, 1, 2, { bound: 1000 });
    const writer = call.upload.getWriter();
    writer.closed.catch(() => {});
    socket.deliver({
      form: "response",
      envelope: { stamp: 0n, seq: 1n },
      target: target("uploadNamed"),
      correlation: 1n,
      outcome: { outcome: "accepted" },
    });
    await call.accepted;
    const { fifth, state } = await spend(writer);
    const abort = writer.abort();
    await tick();
    assert.equal(state.settled, true);
    await assert.rejects(fifth, SessionError);
    await abort;
    const ends = frames(socket).filter((f) => f.form === "stream_end");
    assert.deepEqual(ends.map((f) => f.outcome.kind), ["interrupted"]);
    assert.equal(frames(socket).filter((f) => f.form === "chunk").length, 4);
  } finally {
    session.transport.close();
  }
});
Deno.test("plain_reply_is_verified_before_decode_early_FakeSocket", async () => {
  const fn = uploadFunction("uploadChoice");
  for (const blocked of [false, true]) {
    const { session, socket } = await socketHarness();
    try {
      const call = fn(session, 1, 2, { bound: 1000 });
      const writer = call.upload.getWriter();
      const closed = writer.closed.catch((e) => e);
      const closedState = observe(closed);
      const failed = blocked
        ? (await spend(writer)).fifth.catch((e) => e)
        : undefined;
      const value = { kind: "ok", value: { value: 9 } };
      socket.deliver({
        form: "response",
        envelope: { stamp: 0n, seq: 1n },
        target: target("uploadChoice"),
        correlation: 1n,
        outcome: {
          outcome: "reply",
          payload: entry("uploadChoice").reply!.encode(value),
        },
      });
      assert.deepEqual(await call.reply, value);
      await tick();
      assert.equal(
        closedState.settled,
        true,
        "early reply errors an idle upload too",
      );
      const remote = await closed;
      if (blocked) assert.strictEqual(await failed, remote);
      assert.ok(remote instanceof SessionError);
      assert.equal(remote.failure.category, "interrupted");
      assert.strictEqual(await closed, remote);
      assert.strictEqual(
        await writer.write({ value: 9 }).catch((e) => e),
        remote,
      );
    } finally {
      session.transport.close();
    }
  }
});
