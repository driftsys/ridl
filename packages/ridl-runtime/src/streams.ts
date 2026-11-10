import type { Codec, VerifyResult } from "./codec.ts";
import { CodecError, StreamError } from "./errors.ts";
import { isWellFormedString } from "./well_formed.ts";

/** Options for a streaming interaction. */
export interface StreamOptions {
  /** The response bound in milliseconds. */
  readonly bound: number;
}

/**
 * Byte stream operations supplied by a transport, without a transport-package
 * dependency. Generated functions call these methods with their receiver.
 */
export interface StreamTransport {
  /** Opens a reply stream; cancellation reaches its upstream transport. */
  queryStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: StreamOptions,
  ): ReadableStream<Uint8Array>;
  /**
   * Opens concurrent upload and reply streams. Upload close leaves the reply
   * open; aborting upload or canceling reply interrupts the call.
   */
  queryDuplex(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: StreamOptions,
  ): { upload: WritableStream<Uint8Array>; stream: ReadableStream<Uint8Array> };
  /** Opens an upload whose acceptance does not confirm provider consumption. */
  commandStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: StreamOptions,
  ): { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
  /** Opens an upload and returns the eventual plain reply bytes. */
  queryWithUpload(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: StreamOptions,
  ): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
}

/**
 * The transport and loaded codec used by a generated streaming interaction.
 * Construct as `{ transport, codec }` after loading the codec with the generated
 * catalog hash, payload names and rule texts. See the package README example.
 */
export interface StreamSession {
  readonly transport: StreamTransport;
  readonly codec: Codec;
}

/**
 * Verifies each whole payload before decoding and delivering its value.
 * Local payload failures error the stream with StreamError; missing or unknown
 * verifier rule texts use CodecError. Transport failures retain their identity.
 * Native pipeThrough cancellation and backpressure propagate upstream.
 */
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
      let value: T;
      try {
        value = decode(bytes);
      } catch (error) {
        throw codecException(error);
      }
      controller.enqueue(value);
    },
  });
}

/**
 * Decodes each chunk as complete UTF-8 text, preserving a leading BOM.
 * Invalid UTF-8 errors the transform with StreamError and the utf8 rule.
 */
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

/**
 * Encodes and verifies each named element as one whole payload without splitting
 * it. An oversized element fails with StreamError (corrupt); a failed rule uses
 * StreamError (contract). Local failures abort the raw upload. Each write waits
 * for all resulting raw writes; closing confirms transmission, not consumption.
 * Remote errors retain their identity, except that a close requested after the
 * writer has errored follows the platform TypeError behavior.
 */
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
      throw codecException(error);
    }
    if (bytes.length > limit) throw new StreamError({ category: "corrupt" });
    const result = verify(bytes);
    if (!result.ok) throw verificationError(result);
    await write(bytes);
  });
}

/**
 * Splits raw bytes at the member limit, preserving empty chunks for a positive
 * limit. A nonpositive limit fails with StreamError (corrupt). Writes wait for
 * every raw chunk, and abort reaches a raw write waiting for credit.
 */
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

/**
 * Encodes text as complete UTF-8 chunks and rejects unpaired surrogates with
 * StreamError and the utf8 rule. A code point larger than the limit, or a
 * nonpositive limit, fails as corrupt. Empty writes remain empty chunks for a
 * positive limit. Writes wait for every raw chunk before completing.
 */
export function textUpload(
  upload: WritableStream<Uint8Array>,
  limit: number,
): WritableStream<string> {
  const encoder = new TextEncoder();
  return adaptUpload(upload, async (text, write) => {
    if (!isWellFormedString(text)) {
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

function codecException(error: unknown): StreamError {
  // Each generated module defines its own RuleError class. A generated
  // encode throws the runtime's ConstraintError for a string that is not
  // well-formed; its rule is a payload rule too.
  if (
    error instanceof Error &&
    (error.name === "RuleError" || error.name === "ConstraintError") &&
    "rule" in error
  ) {
    switch (error.rule) {
      case "length":
      case "pattern":
      case "range":
      case "step":
      case "unique":
      case "utf8":
      case "variant":
        return new StreamError({
          category: "contract",
          contract: { category: "invalid-value", rule: error.rule },
        }, { cause: error });
    }
  }
  return new StreamError({ category: "corrupt" }, { cause: error });
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
