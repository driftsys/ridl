import type { Codec, VerifyResult } from "./codec.ts";
import { CodecError, StreamError } from "./errors.ts";

/** Options for a streaming interaction. */
export interface StreamOptions {
  /** The response bound in milliseconds. */
  readonly bound: number;
}

/** Byte stream operations supplied by a transport. */
export interface StreamTransport {
  queryStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: StreamOptions,
  ): ReadableStream<Uint8Array>;
  commandStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: StreamOptions,
  ): { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
  queryWithUpload(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: StreamOptions,
  ): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
}

/** The transport and loaded codec used by a streaming interaction. */
export interface StreamSession {
  readonly transport: StreamTransport;
  readonly codec: Codec;
}

/** Verifies each whole payload before decoding and delivering its value. */
export function decodeStream<T>(
  decode: (bytes: Uint8Array) => T,
  verify: (bytes: Uint8Array) => VerifyResult,
): TransformStream<Uint8Array, T> {
  return new TransformStream({
    transform(bytes, controller) {
      const result = verify(bytes);
      if (!result.ok) {
        if (result.kind === "malformed") {
          throw new StreamError({ category: "corrupt" });
        }
        if (result.kind === "rule") {
          switch (result.rule) {
            case "length":
            case "pattern":
            case "range":
            case "step":
            case "unique":
            case "utf8":
            case "variant":
              throw new StreamError({
                category: "contract",
                contract: { category: "invalid-value", rule: result.rule },
              });
            default:
              throw new CodecError(`unknown payload rule: ${result.rule}`);
          }
        }
        throw new CodecError("unknown payload type during stream verification");
      }
      controller.enqueue(decode(bytes));
    },
  });
}

/** Decodes each chunk as complete UTF-8 text, preserving a leading BOM. */
export function decodeTextStream(): TransformStream<Uint8Array, string> {
  const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  return new TransformStream({
    transform(bytes, controller) {
      let text: string;
      try {
        text = decoder.decode(bytes);
      } catch {
        throw new StreamError({
          category: "contract",
          contract: { category: "invalid-value", rule: "utf8" },
        });
      }
      controller.enqueue(text);
    },
  });
}
