import type { VerifyResult } from "./codec.ts";
import { CallError } from "./errors.ts";
import {
  exceptionFailure,
  ruleError,
  verificationFailure,
} from "./failures.ts";
import { isWellFormedString } from "./well_formed.ts";

/**
 * Verifies each whole payload before decoding and delivering its value.
 * Local payload failures error the stream with CallError; missing or unknown
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
      if (!result.ok) throw verificationFailure(result);
      let value: T;
      try {
        value = decode(bytes);
      } catch (error) {
        throw exceptionFailure(error);
      }
      controller.enqueue(value);
    },
  });
}

/**
 * Decodes each chunk as complete UTF-8 text, preserving a leading BOM.
 * Invalid UTF-8 errors the transform with CallError and the utf8 rule.
 */
export function decodeTextStream(): TransformStream<Uint8Array, string> {
  const decoder = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
  return new TransformStream({
    transform(bytes, controller) {
      let text: string;
      try {
        text = decoder.decode(bytes);
      } catch {
        throw ruleError("utf8");
      }
      controller.enqueue(text);
    },
  });
}

/**
 * Encodes and verifies each named element as one whole payload without splitting
 * it. An oversized element fails with CallError (corrupt); a failed rule uses
 * CallError (contract). Local failures abort the raw upload. Each write waits
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
      throw exceptionFailure(error);
    }
    if (bytes.length > limit) throw new CallError({ category: "corrupt" });
    const result = verify(bytes);
    if (!result.ok) throw verificationFailure(result);
    await write(bytes);
  });
}

/**
 * Splits raw bytes at the member limit, preserving empty chunks for a positive
 * limit. A nonpositive limit fails with CallError (corrupt). Writes wait for
 * every raw chunk, and abort reaches a raw write waiting for credit.
 */
export function byteUpload(
  upload: WritableStream<Uint8Array>,
  limit: number,
): WritableStream<Uint8Array> {
  return adaptUpload(upload, async (bytes, write) => {
    if (limit <= 0) throw new CallError({ category: "corrupt" });
    if (bytes.length === 0) await write(bytes);
    for (let start = 0; start < bytes.length; start += limit) {
      await write(bytes.subarray(start, start + limit));
    }
  });
}

/**
 * Encodes text as complete UTF-8 chunks and rejects unpaired surrogates with
 * CallError and the utf8 rule. A code point larger than the limit, or a
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
      throw ruleError("utf8");
    }
    if (limit <= 0) throw new CallError({ category: "corrupt" });
    let chunk = "";
    let size = 0;
    for (const point of text) {
      const length = encoder.encode(point).length;
      if (length > limit) throw new CallError({ category: "corrupt" });
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
