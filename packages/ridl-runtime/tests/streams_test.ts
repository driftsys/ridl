import { assertEquals, assertRejects } from "@std/assert";
import {
  CodecError,
  decodeStream,
  decodeTextStream,
  StreamError,
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
