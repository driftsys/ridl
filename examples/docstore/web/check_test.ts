import assert from "node:assert/strict";
import { checkBytes, checkChunks, checkDuplex, type Chunk } from "./check.ts";

// Expectations are built independently of the consumer's validators.
const document = Uint8Array.from({ length: 204_800 }, (_, i) => i % 251);
function chunks(): Chunk[] {
  return Array.from({ length: 64 }, (_, index) => ({
    index,
    data: document.slice(index * 3200, (index + 1) * 3200),
  }));
}
function stream<T>(values: T[]): ReadableStream<T> {
  return new ReadableStream({
    start(controller) {
      for (const value of values) controller.enqueue(value);
      controller.close();
    },
  });
}

Deno.test("byte transfer requires exact content and count", async () => {
  await checkBytes(stream([document.slice(0, 17), document.slice(17)]));
  const changed = document.slice();
  changed[100_000] ^= 1;
  await assert.rejects(checkBytes(stream([changed])), /byte content/);
  await assert.rejects(
    checkBytes(stream([document.slice(0, -1)])),
    /byte count/,
  );
  await assert.rejects(
    checkBytes(stream([document, Uint8Array.of(1)])),
    /byte count/,
  );
});

Deno.test("named transfers require exact indexed values and normal end", async () => {
  await checkChunks(stream(chunks()));
  const changed = chunks();
  changed[31].data[1599] ^= 1;
  await assert.rejects(checkChunks(stream(changed)), /chunk content/);
  const wrongIndex = chunks();
  wrongIndex[31].index = 32;
  await assert.rejects(checkChunks(stream(wrongIndex)), /chunk order/);
  const short = chunks();
  short[31].data = short[31].data.slice(1);
  await assert.rejects(checkChunks(stream(short)), /chunk size/);
  const dropped = chunks();
  dropped.splice(31, 1);
  await assert.rejects(checkChunks(stream(dropped)), /chunk order/);
  const duplicated = chunks();
  duplicated.splice(31, 0, duplicated[31]);
  await assert.rejects(checkChunks(stream(duplicated)), /chunk order/);
  const reordered = chunks();
  [reordered[31], reordered[32]] = [reordered[32], reordered[31]];
  await assert.rejects(checkChunks(stream(reordered)), /chunk order/);
  await assert.rejects(
    checkChunks(stream(chunks().slice(0, 63))),
    /chunk count/,
  );
  await assert.rejects(
    checkChunks(stream([...chunks(), chunks()[0]])),
    /chunk count/,
  );
  const unfinished = chunks();
  await assert.rejects(
    checkChunks(
      new ReadableStream({
        pull(controller) {
          const next = unfinished.shift();
          if (next) controller.enqueue(next);
          else controller.error(new Error("interrupted before normal end"));
        },
      }),
    ),
    /interrupted/,
  );
});

// Platform streams provide the validator's input, without sockets or codecs.
type Corruption = "dropped" | "duplicated" | "reordered" | "modified";
function echo(
  options: {
    earlyEnd?: "immediate" | "deferred";
    corruption?: Corruption;
    corruptionIndex?: number;
  } = {},
) {
  let output!: ReadableStreamDefaultController<Chunk>;
  const reply = new ReadableStream<Chunk>({
    start(c) {
      output = c;
    },
  });
  let count = 0;
  let held: Chunk | undefined;
  let ended = false;
  const upload = new WritableStream<Chunk>({
    write(value) {
      if (ended) return;
      // Model an end already sent by the provider but delivered only on the
      // next operation. A fixed wait cannot establish that no end was sent.
      if (count === 64 && options.earlyEnd === "deferred") {
        output.close();
        ended = true;
        return;
      }
      count++;
      if (value.index === (options.corruptionIndex ?? 31)) {
        switch (options.corruption) {
          case "dropped":
            return;
          case "duplicated":
            output.enqueue(value);
            break;
          case "reordered":
            held = value;
            return;
          case "modified": {
            const data = value.data.slice();
            data[1599] ^= 1;
            value = { ...value, data };
            break;
          }
        }
      }
      output.enqueue(value);
      if (held) {
        output.enqueue(held);
        held = undefined;
      }
      if (
        count === 64 &&
        (options.earlyEnd === "immediate" || options.corruption === "dropped")
      ) {
        output.close();
        ended = true;
      }
    },
    close() {
      if (!ended) output.close();
    },
  });
  return { upload, stream: reply };
}

// Index 64 follows the 204800-byte document and has another full block.
function duplexChunk(index: number): Chunk {
  return {
    index,
    data: Uint8Array.from({ length: 3200 }, (_, i) => (index * 3200 + i) % 251),
  };
}

Deno.test("duplex rejects dropped, duplicated, reordered and modified replies", async (t) => {
  for (
    const corruption of [
      "dropped",
      "duplicated",
      "reordered",
      "modified",
    ] as const
  ) {
    await t.step(corruption, async () => {
      await assert.rejects(checkDuplex(echo({ corruption }), duplexChunk));
    });
  }
});

Deno.test("duplex requires reply end after upload close", async () => {
  await checkDuplex(echo(), duplexChunk);
  await assert.rejects(
    checkDuplex(echo({ earlyEnd: "immediate" }), duplexChunk),
    /before upload close/,
  );
});

Deno.test("duplex requires the 65th echo before closing upload even with a deferred end", async () => {
  await assert.rejects(
    checkDuplex(echo({ earlyEnd: "deferred" }), duplexChunk),
    /before upload close/,
  );
});

Deno.test("duplex checks the content of the 65th echo", async () => {
  await assert.rejects(
    checkDuplex(
      echo({ corruption: "modified", corruptionIndex: 64 }),
      duplexChunk,
    ),
    /chunk content/,
  );
});
