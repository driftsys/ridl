/** A named chunk as observed by the consumer. */
export interface Chunk {
  index: number;
  data: Uint8Array;
}

/** Checks the complete byte transfer, including its explicit end. */
export async function checkBytes(
  stream: ReadableStream<Uint8Array>,
): Promise<void> {
  let offset = 0;
  for await (const bytes of stream) {
    if (offset + bytes.length > 204_800) {
      throw new Error("byte count exceeds 204800");
    }
    for (const byte of bytes) {
      if (byte !== offset % 251) {
        throw new Error(`byte content differs at ${offset}`);
      }
      offset++;
    }
  }
  if (offset !== 204_800) {
    throw new Error(`byte count ${offset}, expected 204800`);
  }
}

/** Checks the complete ordered named transfer, including its explicit end. */
export async function checkChunks(
  stream: ReadableStream<Chunk>,
  expectedCount = 64,
): Promise<void> {
  let count = 0;
  for await (const chunk of stream) {
    if (count >= expectedCount) {
      throw new Error(`chunk count exceeds ${expectedCount}`);
    }
    if (chunk.index !== count) {
      throw new Error(`chunk order: expected ${count}, got ${chunk.index}`);
    }
    if (chunk.data.length !== 3200) {
      throw new Error(`chunk size at ${count} is not 3200`);
    }
    for (let i = 0; i < chunk.data.length; i++) {
      if (chunk.data[i] !== (count * 3200 + i) % 251) {
        throw new Error(`chunk content differs at ${count}:${i}`);
      }
    }
    count++;
  }
  if (count !== expectedCount) {
    throw new Error(`chunk count ${count}, expected ${expectedCount}`);
  }
}

/** Checks a concurrent echo, including progress before close and end after it. */
export async function checkDuplex<T extends Chunk>(
  exchange: { upload: WritableStream<T>; stream: ReadableStream<T> },
  chunk: (index: number) => T,
): Promise<void> {
  const writer = exchange.upload.getWriter();
  const reader = exchange.stream.getReader();
  try {
    await writer.write(chunk(0));
    const first = await reader.read();
    if (first.done) throw new Error("duplex ended before 64 replies");
    const values = [first.value];
    // The first reply must arrive while the upload is still open. The other
    // 63 writes and reads proceed concurrently so that credit can circulate.
    await Promise.all([
      (async () => {
        for (let i = 1; i < 64; i++) await writer.write(chunk(i));
      })(),
      (async () => {
        for (let i = 1; i < 64; i++) {
          const next = await reader.read();
          if (next.done) throw new Error("duplex ended before 64 replies");
          values.push(next.value);
        }
      })(),
    ]);
    // A reply that ended after 64 echoes cannot answer another write. Require
    // its echo while upload is open, then require normal end after close.
    await writer.write(chunk(64));
    const probe = await reader.read();
    if (probe.done) throw new Error("duplex ended before upload close");
    values.push(probe.value);
    await checkChunks(
      new ReadableStream({
        start(controller) {
          for (const value of values) controller.enqueue(value);
          controller.close();
        },
      }),
      65,
    );
    await writer.close();
    const end = await reader.read();
    if (!end.done) throw new Error("duplex chunk count exceeds 65");
  } finally {
    writer.releaseLock();
    reader.releaseLock();
  }
}
