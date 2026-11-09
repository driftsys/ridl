import assert from "node:assert/strict";
import { connect, Kind, SessionError } from "../mod.ts";
import {
  attach,
  decode,
  encode,
  encodingTag,
  FRAME_VERSION,
  SUBPROTOCOL,
} from "../src/frame.ts";
import type { Frame } from "../src/frame.ts";
import type { Note, Occurrence, Sample, Session } from "../mod.ts";
import * as catalog from "./catalog.ts";
import { startServer } from "./server.ts";
import { waiter } from "./waiter.ts";

Deno.test("a Deno program receives a live signal from the Rust server", async () => {
  const server = await startServer();
  const notes: Note[] = [];
  const init = waiter<Sample>("init publish", 2000);
  const live = waiter<Sample>("live publish", 2000);
  const occurred = waiter<Occurrence>("occurrence", 2000);
  try {
    const session = await connect(`ws://127.0.0.1:${server.port}`, {
      catalog: { name: catalog.NAME, hash: catalog.HASH },
      encoding: catalog.ENCODING,
      onPublish(target, sample) {
        if (target.iface !== catalog.IFACE || target.ord !== catalog.SIGNAL) {
          return;
        }
        if (sample.provenance === "init") init.accept(sample);
        if (sample.provenance === "live") live.accept(sample);
      },
      onOccurrence(occurrence) {
        if (
          occurrence.iface === catalog.IFACE &&
          occurrence.ord === catalog.EVENT
        ) {
          occurred.accept(occurrence);
        }
      },
      onNote(note) {
        notes.push(note);
      },
    });

    const fixed = await session.read({
      kind: Kind.Fixed,
      iface: catalog.IFACE,
      ord: catalog.FIXED,
    });
    assert.equal(fixed.provenance, "live");
    assert.deepEqual(fixed.payload, catalog.FIXED_VALUE);
    assert.equal(fixed.envelope.seq, 0n);

    // The large values cross in blocks or whole, and every byte is checked.
    for (const [ord, length] of catalog.FIXED_LENGTHS) {
      const large = await session.read({
        kind: Kind.Fixed,
        iface: catalog.IFACE,
        ord,
      });
      assert.equal(large.provenance, "live");
      assert.equal(large.payload?.length, length);
      assert.deepEqual(large.payload, catalog.pattern(length));
    }
    assert.deepEqual(notes.filter((n) => n.note === "discarded"), []);

    init.start();
    await session.subscribe({
      kind: Kind.Signal,
      iface: catalog.IFACE,
      ord: catalog.SIGNAL,
    });
    const held = await init.promise;
    assert.equal(held.envelope.seq, 0n);
    assert.equal(held.payload, null);
    assert.deepEqual(session.sample(catalog.IFACE, catalog.SIGNAL), held);

    await session.subscribe({
      kind: Kind.Event,
      iface: catalog.IFACE,
      ord: catalog.EVENT,
    });

    const args = Uint8Array.of(0x2a, 0x07);
    // The waits were registered with the listeners at connect; their time
    // limits count from the command.
    live.start();
    occurred.start();
    await session.command(catalog.IFACE, catalog.COMMAND, args, {
      bound: 2000,
    });

    const sample = await live.promise;
    assert.deepEqual(sample.payload, args);
    assert.ok(sample.envelope.seq >= 1n, `seq ${sample.envelope.seq}`);

    const occurrence = await occurred.promise;
    assert.deepEqual(occurrence.payload, args);

    const reply = await session.query(
      catalog.IFACE,
      catalog.QUERY,
      Uint8Array.of(0x01, 0x80, 0xff),
      { bound: 2000 },
    );
    assert.deepEqual(reply, Uint8Array.of(0x02, 0x81, 0x00));

    session.close();
    const ended = await session.closed;
    assert.equal(ended.code, 1000);
    assert.deepEqual(notes.filter((n) => n.note === "violation"), []);
  } finally {
    init.cancel();
    live.cancel();
    occurred.cancel();
    await server.stop();
  }
});

Deno.test("a signal check refuses a live publish, keeps the last good bytes and recovers", async () => {
  const server = await startServer();
  const notes: Note[] = [];
  let next = waiter<Sample>("publish", 2000);
  try {
    const session = await connect(`ws://127.0.0.1:${server.port}`, {
      catalog: { name: catalog.NAME, hash: catalog.HASH },
      encoding: catalog.ENCODING,
      checks: catalog.SIGNAL_CHECKS,
      onPublish(target, sample) {
        if (target.iface === catalog.IFACE && target.ord === catalog.SIGNAL) {
          next.accept(sample);
        }
      },
      onNote(note) {
        notes.push(note);
      },
    });
    const signal = {
      kind: Kind.Signal,
      iface: catalog.IFACE,
      ord: catalog.SIGNAL,
    };
    /** Sends a command whose argument the provider copies to the signal. */
    const publishAfter = async (args: Uint8Array): Promise<Sample> => {
      next = waiter<Sample>("publish", 2000);
      next.start();
      await session.command(catalog.IFACE, catalog.COMMAND, args, {
        bound: 2000,
      });
      return await next.promise;
    };

    next.start();
    await session.subscribe(signal, { bound: 2000 });
    const init = await next.promise;
    assert.equal(init.provenance, "init");
    assert.equal(init.lastGood, null);

    const good = await publishAfter(catalog.GOOD_FIRST);
    assert.equal(good.provenance, "live");
    assert.deepEqual(good.payload, catalog.GOOD_FIRST);
    assert.deepEqual(good.lastGood, catalog.GOOD_FIRST);
    assert.equal(good.detected, undefined);

    const refused = await publishAfter(catalog.REFUSED);
    assert.equal(refused.provenance, "invalid");
    assert.equal(refused.payload, null);
    assert.deepEqual(refused.detected, { rule: "range" });
    assert.deepEqual(refused.lastGood, catalog.GOOD_FIRST);
    assert.deepEqual(session.sample(catalog.IFACE, catalog.SIGNAL), refused);

    // The provider still holds the refused bytes, so a read is answered with
    // them and resolves with the invalid sample instead of rejecting.
    const read = await session.read(signal, { bound: 2000 });
    assert.equal(read.provenance, "invalid");
    assert.equal(read.payload, null);
    assert.deepEqual(read.detected, { rule: "range" });
    assert.deepEqual(read.lastGood, catalog.GOOD_FIRST);

    const recovered = await publishAfter(catalog.GOOD_SECOND);
    assert.equal(recovered.provenance, "live");
    assert.deepEqual(recovered.payload, catalog.GOOD_SECOND);
    assert.deepEqual(recovered.lastGood, catalog.GOOD_SECOND);
    assert.equal(recovered.detected, undefined);
    assert.deepEqual(
      session.sample(catalog.IFACE, catalog.SIGNAL),
      recovered,
    );

    session.close();
    const ended = await session.closed;
    assert.equal(ended.code, 1000);
    assert.deepEqual(
      notes.filter((n) => n.note === "violation" || n.note === "check-failed"),
      [],
    );
  } finally {
    next.cancel();
    await server.stop();
  }
});

Deno.test("queryStream_receives_64_elements_over_loopback_server", async () => {
  const server = await startServer();
  const notes: Note[] = [];
  try {
    const session = await connect(`ws://127.0.0.1:${server.port}`, {
      catalog: { name: catalog.NAME, hash: catalog.HASH },
      encoding: catalog.ENCODING,
      onNote(note) {
        notes.push(note);
      },
    });
    const stream = session.queryStream(
      catalog.IFACE,
      catalog.STREAM_QUERY,
      Uint8Array.of(64),
      { bound: 2000 },
    );
    // More chunks than the initial credit of 4: the reply arrives in full
    // only if the credit the reader grants reaches the server. Without that
    // credit the server waits, so the read has a time limit.
    let timer: ReturnType<typeof setTimeout> | undefined;
    const late = new Promise<never>((_, reject) => {
      timer = setTimeout(
        () => reject(new Error("the stream did not end within 5 s")),
        5000,
      );
    });
    try {
      assert.deepEqual(
        await Promise.race([catalog.readElements(stream), late]),
        { count: 64, match: true },
      );
    } finally {
      clearTimeout(timer);
    }
    session.close();
    const ended = await session.closed;
    assert.equal(ended.code, 1000);
    assert.deepEqual(notes, []);
  } finally {
    await server.stop();
  }
});

/** Runs one upload scenario with a time limit and closes its session on failure. */
async function withUploadSession(
  run: (session: Session) => Promise<void>,
): Promise<void> {
  const server = await startServer();
  try {
    const notes: Note[] = [];
    const session = await connect(`ws://127.0.0.1:${server.port}`, {
      catalog: { name: catalog.NAME, hash: catalog.HASH },
      encoding: catalog.ENCODING,
      onNote(note) {
        notes.push(note);
      },
    });
    let timer: ReturnType<typeof setTimeout> | undefined;
    const late = new Promise<never>((_, reject) => {
      timer = setTimeout(
        () =>
          reject(new Error("the upload scenario did not finish within 5 s")),
        5000,
      );
    });
    try {
      await Promise.race([run(session), late]);
      // A query refusal can send both a stream end and a response; the
      // client discards the latter after settling on the stream end.
      assert.deepEqual(notes.filter((note) => note.note === "violation"), []);
    } finally {
      clearTimeout(timer);
      session.close();
      await session.closed;
    }
  } finally {
    await server.stop();
  }
}

Deno.test("queryDuplex_echoes_64_chunks", async () => {
  await withUploadSession(async (session) => {
    const { upload, stream } = session.queryDuplex(
      catalog.IFACE,
      catalog.DUPLEX,
      new Uint8Array(),
      { bound: 2000 },
    );
    const writer = upload.getWriter();
    const reader = stream.getReader();
    writer.closed.catch(() => {});
    reader.closed.catch(() => {});
    const element = (i: number) =>
      Uint8Array.from({ length: 1024 }, (_, j) => (i + j) % 251);
    try {
      await writer.write(element(0));
      // The first reply must arrive while the upload is still open.
      assert.deepEqual(await reader.read(), { done: false, value: element(0) });
      await Promise.all([
        (async () => {
          for (let i = 1; i < 64; i++) await writer.write(element(i));
          await writer.close();
        })(),
        (async () => {
          for (let i = 1; i < 64; i++) {
            assert.deepEqual(await reader.read(), {
              done: false,
              value: element(i),
            });
          }
          assert.equal((await reader.read()).done, true);
        })(),
      ]);
    } finally {
      writer.releaseLock();
      reader.releaseLock();
    }
  });
});

Deno.test("queryDuplex_cancel_and_abort", async () => {
  for (const action of ["cancel", "abort"] as const) {
    await withUploadSession(async (session) => {
      const { upload, stream } = session.queryDuplex(
        catalog.IFACE,
        catalog.DUPLEX,
        new Uint8Array(),
        { bound: 2000 },
      );
      const writer = upload.getWriter();
      const reader = stream.getReader();
      writer.closed.catch(() => {});
      reader.closed.catch(() => {});
      try {
        await writer.write(Uint8Array.of(9));
        assert.deepEqual((await reader.read()).value, Uint8Array.of(9));
        const interrupted = assert.rejects(
          action === "cancel" ? writer.closed : reader.closed,
          (error: unknown) => {
            assert.ok(error instanceof SessionError);
            assert.equal(error.failure.category, "interrupted");
            return true;
          },
        );
        if (action === "cancel") await reader.cancel();
        else await writer.abort();
        await interrupted;
        assert.deepEqual(
          await session.query(
            catalog.IFACE,
            catalog.QUERY,
            Uint8Array.of(1),
            { bound: 2000 },
          ),
          Uint8Array.of(2),
        );
      } finally {
        writer.releaseLock();
        reader.releaseLock();
      }
    });
  }
});

Deno.test("commandStream_uploads_200_000_bytes_and_fetch_reads_them_back", async () => {
  await withUploadSession(async (session) => {
    const bytes = catalog.pattern(catalog.UPLOAD_LEN);
    const { upload, accepted } = session.commandStream(
      catalog.IFACE,
      catalog.UPLOAD,
      Uint8Array.of(1, 0),
      { bound: 2000 },
    );
    await accepted;
    await catalog.writeAll(upload, bytes, catalog.FETCH_CHUNK);
    const fetched = await catalog.readAll(session.queryStream(
      catalog.IFACE,
      catalog.FETCH,
      Uint8Array.of(1),
      { bound: 2000 },
    ));
    assert.equal(fetched.length, 200_000);
    assert.deepEqual(fetched, bytes);
  });
});

Deno.test("queryWithUpload_replies_the_count_and_the_sum_of_200_000_bytes", async () => {
  await withUploadSession(async (session) => {
    const bytes = catalog.pattern(catalog.UPLOAD_LEN);
    const { upload, reply } = session.queryWithUpload(
      catalog.IFACE,
      catalog.UPLOAD_QUERY,
      Uint8Array.of(0),
      { bound: 2000 },
    );
    // Observe a refusal even if it arrives before writeAll finishes.
    reply.catch(() => {});
    await catalog.writeAll(upload, bytes, catalog.FETCH_CHUNK);
    const result = await reply;
    assert.equal(result.length, 12);
    const view = new DataView(
      result.buffer,
      result.byteOffset,
      result.byteLength,
    );
    assert.equal(view.getBigUint64(0, true), 200_000n);
    const sum = bytes.reduce((sum, byte) => (sum + byte) >>> 0, 0);
    assert.equal(view.getUint32(8, true), sum);
  });
});

Deno.test("a_refused_third_block_rejects_the_next_write_with_contract_and_accepted_stays_resolved", async () => {
  await withUploadSession(async (session) => {
    const { upload, accepted } = session.commandStream(
      catalog.IFACE,
      catalog.UPLOAD,
      Uint8Array.of(2, catalog.MODE_REFUSE_RANGE),
      { bound: 2000 },
    );
    await accepted;
    const writer = upload.getWriter();
    let written = 0;
    try {
      await assert.rejects(async () => {
        // Keep writing without closing: a refusal of the third chunk must
        // reach a later write, including one waiting for credit.
        for (; written < 16; written++) {
          await writer.write(catalog.pattern(1024));
        }
      }, (error: unknown) => {
        assert.ok(error instanceof SessionError);
        assert.deepEqual(error.failure, {
          category: "contract",
          contract: { category: "invalid-value", rule: "range" },
        });
        return true;
      });
      assert.ok(written >= 3, `${written} writes succeeded before the refusal`);
      await accepted;
    } finally {
      writer.releaseLock();
    }
  });
});

Deno.test("a_utf8_refusal_crosses_with_rule_utf8", async () => {
  await withUploadSession(async (session) => {
    const { upload, reply } = session.queryWithUpload(
      catalog.IFACE,
      catalog.UPLOAD_QUERY,
      Uint8Array.of(catalog.MODE_REFUSE_UTF8),
      { bound: 2000 },
    );
    const isUtf8Refusal = (error: unknown): boolean => {
      assert.ok(error instanceof SessionError);
      assert.deepEqual(error.failure, {
        category: "contract",
        contract: { category: "invalid-value", rule: "utf8" },
      });
      return true;
    };
    // Register both rejection observers before writing. Do not close the
    // upload: sending the end may stop the server observing a late refusal.
    const refusedReply = assert.rejects(reply, isUtf8Refusal);
    const writer = upload.getWriter();
    try {
      const refusedWrite = assert.rejects(async () => {
        for (let i = 0; i < 16; i++) {
          await writer.write(catalog.pattern(1024));
        }
      }, isUtf8Refusal);
      await Promise.all([refusedReply, refusedWrite]);
    } finally {
      writer.releaseLock();
    }
  });
});

Deno.test("the server answers a read of a fixed in one publish up to 16,137 bytes and in blocks above", async () => {
  const server = await startServer();
  let raw: WebSocket | undefined;
  try {
    const socket = new WebSocket(
      `ws://127.0.0.1:${server.port}`,
      SUBPROTOCOL,
    );
    raw = socket;
    socket.binaryType = "arraybuffer";
    const received: Frame[] = [];
    let wake: (() => void) | undefined;
    socket.onmessage = (event) => {
      const decoded = decode(new Uint8Array(event.data as ArrayBuffer));
      assert.ok(decoded.ok, "the server sends a frame the codec reads");
      received.push(decoded.frame);
      wake?.();
    };
    await new Promise<void>((resolve, reject) => {
      socket.onopen = () => resolve();
      socket.onerror = () => reject(new Error("the connection failed"));
    });
    assert.equal(socket.protocol, SUBPROTOCOL);
    /** Waits for `count` frames more than `from`, up to 2 s. */
    const frames = async (from: number, count: number): Promise<Frame[]> => {
      const deadline = Date.now() + 2000;
      while (received.length < from + count && Date.now() < deadline) {
        await new Promise<void>((resolve) => {
          wake = resolve;
          setTimeout(resolve, 50);
        });
      }
      return received.slice(from);
    };
    socket.send(encode(attach(
      FRAME_VERSION,
      encodingTag(catalog.ENCODING),
      catalog.HASH,
      catalog.NAME,
    )));
    const attached = await frames(0, 1);
    assert.deepEqual(attached.map((f) => f.form), ["attached"]);

    const target = (ord: number) => ({
      kind: Kind.Fixed,
      iface: catalog.IFACE,
      ord,
    });
    // 16,137 bytes is the most one publish carries: one frame.
    socket.send(encode({ form: "read", target: target(catalog.FIXED_WHOLE) }));
    const whole = await frames(1, 1);
    // Wait for any further frame, so that a split value is counted in full.
    await new Promise((resolve) => setTimeout(resolve, 200));
    const wholeAll = received.slice(1);
    assert.equal(whole.length, 1);
    assert.deepEqual(wholeAll.map((f) => f.form), ["publish"]);

    // 16,138 bytes is one more: ceil(16,138 / 4,096) = 4 blocks.
    const mark = received.length;
    socket.send(encode({ form: "read", target: target(catalog.FIXED_SPLIT) }));
    await frames(mark, 4);
    await new Promise((resolve) => setTimeout(resolve, 200));
    const split = received.slice(mark);
    assert.deepEqual(split.map((f) => f.form), Array(4).fill("block"));
    const blocks = split.map((f) => {
      assert.equal(f.form, "block");
      return f as Extract<Frame, { form: "block" }>;
    });
    assert.deepEqual(blocks.map((b) => b.index), [0, 1, 2, 3]);
    assert.deepEqual(blocks.map((b) => b.total), Array(4).fill(16138));
    assert.deepEqual(blocks.map((b) => b.payload.length), [
      4096,
      4096,
      4096,
      3850,
    ]);
  } finally {
    // Close the raw socket and wait for it to end before the server stops.
    if (raw !== undefined && raw.readyState !== WebSocket.CLOSED) {
      const closed = new Promise<void>((resolve) => {
        raw!.onclose = () => resolve();
      });
      raw.close();
      await closed;
    }
    await server.stop();
  }
});
