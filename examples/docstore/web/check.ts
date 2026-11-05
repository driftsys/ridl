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
): Promise<void> {
  let count = 0;
  for await (const chunk of stream) {
    if (count >= 64) throw new Error("chunk count exceeds 64");
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
  if (count !== 64) throw new Error(`chunk count ${count}, expected 64`);
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
    await checkChunks(
      new ReadableStream({
        start(controller) {
          for (const value of values) controller.enqueue(value);
          controller.close();
        },
      }),
    );
    let settled = false;
    const end = reader.read().then(
      (value) => {
        settled = true;
        return { value };
      },
      (error: unknown) => {
        settled = true;
        return { error };
      },
    );
    // Five default server polling intervals leave the upload open while the
    // extra read checks for an early end or an unsolicited 65th reply. The
    // overall program deadline also bounds a provider that never ends.
    await new Promise((resolve) => setTimeout(resolve, 100));
    if (settled) throw new Error("duplex replied or ended before upload close");
    await writer.close();
    const result = await end;
    if ("error" in result) throw result.error;
    if (!result.value.done) throw new Error("duplex chunk count exceeds 64");
  } finally {
    writer.releaseLock();
    reader.releaseLock();
  }
}
