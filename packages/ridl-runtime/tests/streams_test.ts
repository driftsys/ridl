import { assertEquals, assertRejects, assertStrictEquals } from "@std/assert";
import {
  byteUpload,
  CodecError,
  decodeStream,
  decodeTextStream,
  encodeUpload,
  StreamError,
  textUpload,
  type VerifyResult,
} from "../mod.ts";

Deno.test("decodes_64_verified_elements", async () => {
  const calls: string[] = [];
  const values = await Array.fromAsync(
    ReadableStream.from(
      Array.from({ length: 64 }, (_, n) => Uint8Array.of(n)),
    ).pipeThrough(decodeStream(
      (bytes) => {
        calls.push(`decode ${bytes[0]}`);
        return bytes[0];
      },
      (bytes) => {
        calls.push(`verify ${bytes[0]}`);
        return { ok: true };
      },
    )),
  );
  assertEquals(values, Array.from({ length: 64 }, (_, n) => n));
  assertEquals(
    calls,
    Array.from({ length: 64 }, (_, n) => [`verify ${n}`, `decode ${n}`]).flat(),
  );
});

Deno.test("invalid_payload_never_reaches_decode", async (t) => {
  const cases: { result: VerifyResult; failure?: StreamError["failure"] }[] = [
    {
      result: { ok: false, kind: "malformed", code: 1 },
      failure: { category: "corrupt" },
    },
    ...([
      "length",
      "pattern",
      "range",
      "step",
      "unique",
      "utf8",
      "variant",
    ] as const).map(
      (rule, index) => ({
        result: {
          ok: false as const,
          kind: "rule" as const,
          code: index + 2,
          rule,
        },
        failure: {
          category: "contract" as const,
          contract: { category: "invalid-value" as const, rule },
        },
      }),
    ),
    { result: { ok: false, kind: "unknown-type", code: -1 } },
    { result: { ok: false, kind: "rule", code: 9, rule: "future-rule" } },
    { result: { ok: false, kind: "rule", code: 9 } },
  ];
  for (const { result, failure } of cases) {
    await t.step(JSON.stringify(result), async () => {
      let decodes = 0;
      const reader = ReadableStream.from([Uint8Array.of(1)])
        .pipeThrough(decodeStream(
          () => {
            decodes++;
            return 1;
          },
          () => result,
        )).getReader();
      if (failure) {
        const error = await assertRejects(() => reader.read(), StreamError);
        assertEquals(error.failure, failure);
      } else {
        await assertRejects(() => reader.read(), CodecError);
      }
      assertEquals(decodes, 0);
    });
  }
});

Deno.test("text_chunks_are_independently_valid_utf8", async (t) => {
  await t.step("ASCII, four-byte character, BOM and empty chunk", async () => {
    const chunks = [
      Uint8Array.of(65),
      Uint8Array.of(0xf0, 0x9f, 0x98, 0x80),
      Uint8Array.of(0xef, 0xbb, 0xbf, 66),
      new Uint8Array(),
    ];
    assertEquals(
      await Array.fromAsync(
        ReadableStream.from(chunks).pipeThrough(decodeTextStream()),
      ),
      ["A", "😀", "\ufeffB", ""],
    );
  });
  for (
    const chunks of [
      [Uint8Array.of(0xff)],
      [Uint8Array.of(0xf0, 0x9f), Uint8Array.of(0x98, 0x80)],
      [Uint8Array.of(0x98, 0x80)],
    ]
  ) {
    await t.step(JSON.stringify(chunks), async () => {
      const error = await assertRejects(
        () =>
          Array.fromAsync(
            ReadableStream.from(chunks).pipeThrough(decodeTextStream()),
          ),
        StreamError,
      );
      assertEquals(error.failure, {
        category: "contract",
        contract: { category: "invalid-value", rule: "utf8" },
      });
    });
  }
});

Deno.test("transform_cancel_and_error_cancel_upstream", async (t) => {
  for (const fail of [false, true]) {
    await t.step(
      fail ? "verification failure" : "reader cancellation",
      async () => {
        let cancels = 0;
        let decodes = 0;
        const source = new ReadableStream<Uint8Array>({
          pull(controller) {
            controller.enqueue(Uint8Array.of(1));
          },
          cancel() {
            cancels++;
          },
        }, { highWaterMark: 0 });
        const reader = source.pipeThrough(decodeStream(
          () => {
            decodes++;
            return 1;
          },
          () => ({ ok: false, kind: "malformed", code: 1 }),
        )).getReader();
        if (fail) {
          await assertRejects(() => reader.read(), StreamError);
        } else {
          await reader.cancel();
        }
        await new Promise<void>((resolve) => setTimeout(resolve, 0));
        assertEquals(cancels, 1);
        assertEquals(decodes, 0);
      },
    );
  }
});

Deno.test("idle_reader_does_not_drain_source", async () => {
  let pulls = 0;
  const source = new ReadableStream<Uint8Array>({
    pull(controller) {
      controller.enqueue(Uint8Array.of(pulls++));
      if (pulls === 64) controller.close();
    },
  }, { highWaterMark: 0 });
  const output = source.pipeThrough(decodeStream(
    (bytes) => bytes[0],
    () => ({ ok: true }),
  ));
  await new Promise<void>((resolve) => setTimeout(resolve, 0));
  assertEquals(pulls < 64, true, `unread output pulled ${pulls} chunks`);
  assertEquals(
    await Array.fromAsync(output),
    Array.from({ length: 64 }, (_, n) => n),
  );
});

const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));
function uploadSink() {
  const chunks: Uint8Array[] = [];
  const aborted: unknown[] = [];
  let ends = 0;
  const upload = new WritableStream<Uint8Array>({
    write(bytes) {
      chunks.push(bytes.slice());
    },
    close() {
      ends++;
    },
    abort(error) {
      aborted.push(error);
    },
  });
  return {
    upload,
    chunks,
    aborted,
    get ends() {
      return ends;
    },
  };
}
const valid = (): VerifyResult => ({ ok: true });

Deno.test("named_upload_is_one_verified_chunk", async () => {
  const sink = uploadSink();
  const checked: number[] = [];
  const writer = encodeUpload(
    sink.upload,
    (n: number) => Uint8Array.of(n),
    (b) => {
      checked.push(b[0]);
      return b[0] > 100
        ? { ok: false, kind: "rule", rule: "range", code: 4 }
        : valid();
    },
    5,
  ).getWriter();
  writer.closed.catch(() => {});
  await writer.write(1);
  await writer.write(2);
  const error = await assertRejects(() => writer.write(101), StreamError);
  assertEquals(error.failure, {
    category: "contract",
    contract: { category: "invalid-value", rule: "range" },
  });
  await assertRejects(() => writer.write(3));
  assertEquals(checked, [1, 2, 101]);
  assertEquals(sink.chunks, [Uint8Array.of(1), Uint8Array.of(2)]);
  assertEquals(sink.aborted.length, 1);
  assertEquals(sink.upload.locked, false);
});

Deno.test("raw_uploads_split_at_member_limit", async () => {
  const sink = uploadSink();
  const writer = byteUpload(sink.upload, 5).getWriter();
  const input = Uint8Array.from({ length: 12 }, (_, n) => n);
  await writer.write(input);
  await writer.write(new Uint8Array());
  await writer.close();
  assertEquals(sink.chunks.map((b) => b.length), [5, 5, 2, 0]);
  assertEquals(Uint8Array.from(sink.chunks.flatMap((b) => [...b])), input);
  const text = uploadSink();
  const strings = textUpload(text.upload, 5).getWriter();
  await strings.write("ab😀cd🌍e");
  await strings.write("");
  await strings.close();
  const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  assertEquals(
    text.chunks.every((bytes) => {
      try {
        decoder.decode(bytes);
        return true;
      } catch {
        return false;
      }
    }),
    true,
    "every transmitted text chunk must contain complete UTF-8 code points",
  );
  assertEquals(text.chunks.map((b) => decoder.decode(b)).join(""), "ab😀cd🌍e");
  assertEquals(text.chunks.map((b) => b.length), [2, 5, 5, 1, 0]);
});

Deno.test("unrepresentable_text_and_elements_abort", async () => {
  for (
    const [value, limit, category] of [["\ud800", 5, "contract"], [
      "😀",
      3,
      "corrupt",
    ]] as const
  ) {
    const sink = uploadSink();
    const writer = textUpload(sink.upload, limit).getWriter();
    writer.closed.catch(() => {});
    const error = await assertRejects(() => writer.write(value), StreamError);
    assertEquals(
      error.failure,
      category === "contract"
        ? { category, contract: { category: "invalid-value", rule: "utf8" } }
        : { category },
    );
    assertEquals(sink.chunks.length, 0);
    assertEquals(sink.aborted.length, 1);
  }
  const sink = uploadSink();
  const writer = encodeUpload(
    sink.upload,
    (bytes: Uint8Array) => bytes,
    valid,
    3,
  ).getWriter();
  writer.closed.catch(() => {});
  await assertRejects(() => writer.write(new Uint8Array(4)), StreamError);
  assertEquals(sink.chunks.length, 0);
  assertEquals(sink.aborted.length, 1);
});

Deno.test("typed_write_waits_for_credit_and_close", async () => {
  const gate = Promise.withResolvers<void>();
  let ends = 0;
  const raw = new WritableStream<Uint8Array>({
    write: () => gate.promise,
    close: () => {
      ends++;
    },
  });
  const writer = encodeUpload(raw, (n: number) => Uint8Array.of(n), valid, 5)
    .getWriter();
  let wrote = false;
  const write = writer.write(1).then(() => {
    wrote = true;
  });
  const close = writer.close();
  await tick();
  assertEquals(wrote, false);
  assertEquals(ends, 0);
  gate.resolve();
  await write;
  await close;
  assertEquals(ends, 1);
  assertEquals(raw.locked, false);
});

// Mutation: detach raw writes instead of awaiting their completion.
for (const form of ["text", "bytes"] as const) {
  Deno.test(`${form}_upload_waits_for_every_raw_write`, async () => {
    const gates = [
      Promise.withResolvers<void>(),
      Promise.withResolvers<void>(),
    ];
    const chunks: Uint8Array[] = [];
    const raw = new WritableStream<Uint8Array>({
      write(bytes) {
        chunks.push(bytes.slice());
        return gates[chunks.length - 1].promise;
      },
    });
    // Observe submissions as well as delivery: the native raw writer queues
    // writes, so delivery alone cannot detect unbounded adapter submissions.
    const getWriter = raw.getWriter.bind(raw);
    let submitted = 0;
    raw.getWriter = () => {
      const writer = getWriter();
      const write = writer.write.bind(writer);
      writer.write = (bytes) => {
        submitted++;
        return write(bytes);
      };
      return writer;
    };
    const writer = form === "text"
      ? textUpload(raw, 4).getWriter()
      : byteUpload(raw, 4).getWriter();
    let settled = false;
    const write = (form === "text"
      ? (writer as WritableStreamDefaultWriter<string>).write("ab😀")
      : (writer as WritableStreamDefaultWriter<Uint8Array>).write(
        Uint8Array.of(0, 1, 2, 3, 4, 5),
      )).finally(() => {
        settled = true;
      });
    try {
      await tick();
      assertEquals(
        submitted,
        1,
        "the adapter must wait before submitting another raw write",
      );
      assertEquals(chunks, [
        form === "text" ? Uint8Array.of(97, 98) : Uint8Array.of(0, 1, 2, 3),
      ]);
      assertEquals(
        settled,
        false,
        "typed write must wait for the first raw write",
      );
      gates[0].resolve();
      await tick();
      assertEquals(submitted, 2);
      assertEquals(
        chunks,
        form === "text"
          ? [Uint8Array.of(97, 98), Uint8Array.of(0xf0, 0x9f, 0x98, 0x80)]
          : [Uint8Array.of(0, 1, 2, 3), Uint8Array.of(4, 5)],
      );
      assertEquals(
        settled,
        false,
        "typed write must wait for the last raw write",
      );
      gates[1].resolve();
      await write;
      assertEquals(settled, true);
    } finally {
      for (const gate of gates) {
        gate.resolve();
      }
      await write;
      await writer.close();
    }
  });
}

Deno.test("remote_refusal_errors_typed_writer", async () => {
  for (const rule of ["range", "utf8"]) {
    for (const queuedClose of [false, true]) {
      const gate = Promise.withResolvers<void>();
      let controller!: WritableStreamDefaultController;
      const raw = new WritableStream<Uint8Array>({
        start(c) {
          controller = c;
        },
        write: () => gate.promise,
      });
      const writer = byteUpload(raw, 5).getWriter();
      const closed = writer.closed.catch((e) => e);
      const remote = { category: "contract", rule };
      const pending = writer.write(Uint8Array.of(1)).catch((e) => e);
      const close = queuedClose ? writer.close().catch((e) => e) : undefined;
      await tick();
      controller.error(remote);
      gate.reject(remote);
      assertStrictEquals(await pending, remote);
      assertStrictEquals(
        await writer.write(Uint8Array.of(2)).catch((e) => e),
        remote,
      );
      if (queuedClose) assertStrictEquals(await close, remote);
      else await assertRejects(() => writer.close(), TypeError);
      assertStrictEquals(await closed, remote);
      assertEquals(raw.locked, false);
    }
    // An idle raw failure must reach the typed controller without another write.
    let controller!: WritableStreamDefaultController;
    const raw = new WritableStream<Uint8Array>({
      start(c) {
        controller = c;
      },
    });
    const writer = byteUpload(raw, 5).getWriter();
    let failure: unknown;
    const closed = writer.closed.catch((e) => {
      failure = e;
    });
    const remote = { category: "contract", rule };
    controller.error(remote);
    await tick();
    assertStrictEquals(failure, remote);
    await closed;
    assertEquals(raw.locked, false);
  }
});

Deno.test("abort_releases_a_credit_waiter", async () => {
  const gate = Promise.withResolvers<void>();
  let aborts = 0;
  const raw = new WritableStream<Uint8Array>({
    start(c) {
      c.signal.addEventListener("abort", () => gate.reject(c.signal.reason));
    },
    write: () => gate.promise,
    abort() {
      aborts++;
    },
  });
  const writer = byteUpload(raw, 5).getWriter();
  writer.closed.catch(() => {});
  const failure = new Error("aborted");
  const pending = writer.write(Uint8Array.of(1)).catch((e) => e);
  await tick();
  const abort = writer.abort(failure);
  await tick();
  assertEquals(
    aborts,
    1,
    "abort reaches the raw sink while the typed write is blocked",
  );
  assertEquals(await pending, failure);
  await abort;
  assertEquals(raw.locked, false);
});

Deno.test("empty_upload_waits_for_raw_credit", async () => {
  const gate = Promise.withResolvers<void>();
  const raw = new WritableStream<Uint8Array>({ write: () => gate.promise });
  const writer = byteUpload(raw, 4).getWriter();
  let settled = false;
  const pending = writer.write(new Uint8Array()).then(() => {
    settled = true;
  });
  try {
    await tick();
    assertEquals(settled, false, "empty writes also wait for raw credit");
  } finally {
    gate.resolve();
    await pending;
    await writer.close();
  }
});
