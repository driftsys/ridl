// The test catalog that the example server provides. These values are defined
// in crates/ridl-transport-ws/tests/support/provider.rs; keep them equal to it.
// The test `the_e2e_catalog_holds_the_ordinals_and_lengths_of_the_test_provider`
// in crates/ridl-transport-ws/tests/ts_ceilings.rs fails when the ordinals 7 to
// 9 or their three lengths differ.

/** The unit name of the test catalog. */
export const NAME = "test.ws";
/** The catalog hash: thirty-two bytes of 7. */
export const HASH = new Uint8Array(32).fill(7);
/**
 * The payload encoding the provider serves. The loopback runtime's default
 * encoding is FlatBuffers, and the server refuses an `attach` that asks for
 * another one.
 */
export const ENCODING = "flatbuffers";
/** The one interface of the catalog. */
export const IFACE = 1;
/** The signal, which the provider sets to each accepted command's argument. */
export const SIGNAL = 1;
/** The event, which the provider raises with each accepted command's argument. */
export const EVENT = 2;
/** The command. */
export const COMMAND = 3;
/** The query, whose reply adds 1 to each argument byte, wrapping at 256. */
export const QUERY = 4;
/** The `fixed`. */
export const FIXED = 5;
/** The value the provider provisions for the `fixed`. */
export const FIXED_VALUE = Uint8Array.of(0xf0, 0x0d);
/** The `fixed` of 65,280 bytes, which crosses the binding in 16 blocks. */
export const FIXED_LARGE = 7;
/** The `fixed` of 16,137 bytes, which crosses the binding in one message. */
export const FIXED_WHOLE = 8;
/** The `fixed` of 16,138 bytes, which crosses the binding in 4 blocks. */
export const FIXED_SPLIT = 9;
/**
 * The query whose reply is a stream. Its argument is one byte `n`; the reply
 * is `n` chunks, chunk `i` from 0 being {@link ELEMENT_LEN} bytes of value
 * `i`, then the end of the stream.
 */
export const STREAM_QUERY = 10;
/** The length of each chunk of the reply of {@link STREAM_QUERY}. */
export const ELEMENT_LEN = 1024;
/** The length of each large `fixed`, by ordinal. */
export const FIXED_LENGTHS: ReadonlyMap<number, number> = new Map([
  [FIXED_LARGE, 65_280],
  [FIXED_WHOLE, 16_137],
  [FIXED_SPLIT, 16_138],
]);

/**
 * Reads the whole reply of {@link STREAM_QUERY} for `n`, and returns how many
 * chunks arrived and whether chunk `i` is {@link ELEMENT_LEN} bytes of value
 * `i` for each `i`. It rejects when the stream errors, so a stream that
 * closes normally is the only one it describes.
 */
export async function readElements(
  stream: ReadableStream<Uint8Array>,
): Promise<{ count: number; match: boolean }> {
  let count = 0;
  let match = true;
  for await (const chunk of stream) {
    const i = count;
    match &&= chunk.length === ELEMENT_LEN && chunk.every((b) => b === i);
    count += 1;
  }
  return { count, match };
}

/** The `n` bytes whose byte `i` is `i mod 251`. */
export function pattern(n: number): Uint8Array {
  return Uint8Array.from({ length: n }, (_, i) => i % 251);
}
