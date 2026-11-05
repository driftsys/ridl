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

/** Encodes and verifies each named element as one whole payload. */
export function encodeUpload<T>(
  upload: WritableStream<Uint8Array>,
  encode: (value: T) => Uint8Array,
  verify: (bytes: Uint8Array) => VerifyResult,
  limit: number,
): WritableStream<T> {
  return adaptUpload(upload, async (value, write) => {
    let bytes: Uint8Array;
    try {
      bytes = encode(value);
    } catch (error) {
      // Generated encoders report enum and enumset constraints structurally.
      if (error instanceof Error && "rule" in error) {
        throw verificationError({
          ok: false,
          kind: "rule",
          rule: String(error.rule),
          code: 0,
        });
      }
      throw new StreamError({ category: "corrupt" });
    }
    if (bytes.length > limit) throw new StreamError({ category: "corrupt" });
    const result = verify(bytes);
    if (!result.ok) throw verificationError(result);
    await write(bytes);
  });
}

/** Splits raw bytes at the member limit, preserving empty chunks. */
export function byteUpload(
  upload: WritableStream<Uint8Array>,
  limit: number,
): WritableStream<Uint8Array> {
  return adaptUpload(upload, async (bytes, write) => {
    if (limit <= 0) throw new StreamError({ category: "corrupt" });
    if (bytes.length === 0) await write(bytes);
    for (let start = 0; start < bytes.length; start += limit) {
      await write(bytes.subarray(start, start + limit));
    }
  });
}

/** Encodes text as complete UTF-8 chunks and rejects unpaired surrogates. */
export function textUpload(
  upload: WritableStream<Uint8Array>,
  limit: number,
): WritableStream<string> {
  const encoder = new TextEncoder();
  return adaptUpload(upload, async (text, write) => {
    if (!text.isWellFormed()) {
      throw new StreamError({
        category: "contract",
        contract: { category: "invalid-value", rule: "utf8" },
      });
    }
    if (limit <= 0) throw new StreamError({ category: "corrupt" });
    let chunk = "";
    let size = 0;
    for (const point of text) {
      const length = encoder.encode(point).length;
      if (length > limit) throw new StreamError({ category: "corrupt" });
      if (size + length > limit) {
        await write(encoder.encode(chunk));
        chunk = "";
        size = 0;
      }
      chunk += point;
      size += length;
    }
    if (chunk.length || text.length === 0) await write(encoder.encode(chunk));
  });
}

function verificationError(result: Exclude<VerifyResult, { ok: true }>): Error {
  if (result.kind === "malformed") {
    return new StreamError({ category: "corrupt" });
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
        return new StreamError({
          category: "contract",
          contract: { category: "invalid-value", rule: result.rule },
        });
      default:
        return new CodecError(`unknown payload rule: ${result.rule}`);
    }
  }
  return new CodecError("unknown payload type during stream verification");
}

function adaptUpload<T>(
  upload: WritableStream<Uint8Array>,
  send: (
    value: T,
    write: (bytes: Uint8Array) => Promise<void>,
  ) => Promise<void>,
): WritableStream<T> {
  const writer = upload.getWriter();
  let aborting: Promise<void> | undefined;
  let released = false;
  const release = () => {
    released = true;
    writer.releaseLock();
  };
  const abort = (reason: unknown) => {
    if (released) return Promise.resolve();
    aborting ??= writer.abort(reason);
    return aborting;
  };
  return new WritableStream<T>({
    start(controller) {
      // A sink's abort callback runs after its in-flight write settles.
      // Forward the signal immediately so the raw writer releases credit waits.
      controller.signal.addEventListener("abort", () => {
        abort(controller.signal.reason).catch(() => {});
      });
      void writer.closed.then(
        release,
        (error) => {
          controller.error(error);
          release();
        },
      );
    },
    async write(value, controller) {
      try {
        await send(value, (bytes) => writer.write(bytes));
      } catch (error) {
        controller.error(error);
        await abort(error);
        throw error;
      }
    },
    close: () => writer.close(),
    abort,
  });
}
