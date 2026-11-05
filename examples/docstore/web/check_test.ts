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

Deno.test("named and duplex transfers require exact indexed values and normal end", async () => {
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
function echo(earlyEnd: boolean) {
  let output!: ReadableStreamDefaultController<Chunk>;
  const reply = new ReadableStream<Chunk>({
    start(c) {
      output = c;
    },
  });
  let count = 0;
  const upload = new WritableStream<Chunk>({
    write(value) {
      output.enqueue(value);
      if (++count === 64 && earlyEnd) output.close();
    },
    close() {
      if (!earlyEnd) output.close();
    },
  });
  return { upload, stream: reply };
}

Deno.test("duplex requires reply end after upload close", async () => {
  const values = chunks();
  await checkDuplex(echo(false), (index) => values[index]);
  await assert.rejects(
    checkDuplex(echo(true), (index) => values[index]),
    /before upload close/,
  );
});
