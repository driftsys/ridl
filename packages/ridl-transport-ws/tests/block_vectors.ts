/**
 * The byte vectors of `block` messages (frame version 2) that the decoder,
 * the encoder and the session tests share (driftsys/ridl#1129). Each hex
 * string is one whole message; spaces are for reading only.
 */

import type { Violation } from "../mod.ts";

/** Bytes of a hex string; spaces and backticks are ignored. */
export function hex(text: string): Uint8Array {
  const digits = text.replace(/[`\s]/g, "");
  if (digits.length % 2 !== 0) throw new Error(`odd hex digits in ${text}`);
  const out = new Uint8Array(digits.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(digits.slice(2 * i, 2 * i + 2), 16);
  }
  return out;
}

/** The `n` bytes whose byte `i` is `i mod 251`. */
export function pattern(n: number): Uint8Array {
  const out = new Uint8Array(n);
  for (let i = 0; i < n; i++) out[i] = i % 251;
  return out;
}

/** The bytes of all `parts`, in order. */
export function concat(...parts: Uint8Array[]): Uint8Array {
  const out = new Uint8Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

/** The lowercase hex of the SHA-256 of `bytes`. */
export async function sha256(bytes: Uint8Array): Promise<string> {
  const copy = new Uint8Array(bytes);
  const digest = new Uint8Array(await crypto.subtle.digest("SHA-256", copy));
  return [...digest].map((b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * The first 26 bytes of every vector `block`: form 11, kind `Fixed`,
 * interface 1, ordinal 5, stamp, seq 0.
 */
const COMMON = "0b 05 01000000 05000000 ab89674523010000 0000000000000000";

const le32 = (v: number): string =>
  [v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, (v >>> 24) & 0xff]
    .map((b) => b.toString(16).padStart(2, "0")).join("");

/** The 34-byte header of a `block` of the vectors, as hex. */
export function blockHeader(total: number, index: number): string {
  return `${COMMON} ${le32(total)} ${le32(index)}`;
}

/** A `block` message of the vectors: the header, then `payload`. */
export function blockMessage(
  total: number,
  index: number,
  payload: Uint8Array,
): Uint8Array {
  return concat(hex(blockHeader(total, index)), payload);
}

/** The vectors that decode. */
export const BLOCK_LAST = hex(
  `${blockHeader(4098, 1)} abcd`,
);
export const BLOCK_ONE = hex(`${blockHeader(1, 0)} ab`);

/** A violation vector: its id, its message and the TypeScript violation. */
export interface BlockViolation {
  id: string;
  message: Uint8Array;
  violation: Violation;
}

/** The vectors that are protocol violations. */
export function blockViolations(): BlockViolation[] {
  const zeros = (n: number) => new Uint8Array(n);
  const row = (
    id: string,
    message: Uint8Array,
    violation: Violation,
  ): BlockViolation => ({
    id,
    message,
    violation,
  });
  const last = (total: number, index: number, bytes: string) =>
    hex(`${blockHeader(total, index)} ${bytes}`);
  return [
    row(
      "bad-kind-signal",
      hex(
        "0b 01 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd",
      ),
      "bad-block",
    ),
    row(
      "bad-kind-6",
      hex(
        "0b 06 01000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd",
      ),
      "unknown-code",
    ),
    row(
      "bad-interface-0",
      hex(
        "0b 05 00000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd",
      ),
      "zero-number",
    ),
    row(
      "bad-total-0",
      hex(
        "0b 05 01000000 05000000 ab89674523010000 0000000000000000 00000000 00000000 abcd",
      ),
      "bad-block",
    ),
    row("bad-no-bytes", hex(blockHeader(4098, 1)), "bad-block"),
    row(
      "bad-cut-33",
      hex(blockHeader(4098, 1)).subarray(0, 33),
      "too-short",
    ),
    row("bad-index-2", last(4098, 2, "abcd"), "bad-block"),
    row("bad-index-max", last(4098, 0xffffffff, "abcd"), "bad-block"),
    row("bad-short-first", last(4098, 0, "abcd"), "bad-block"),
    row("bad-last-long", last(4098, 1, "abcdef"), "bad-block"),
    row("bad-last-short", last(4098, 1, "ab"), "bad-block"),
    row("bad-first-long", blockMessage(8193, 0, zeros(4097)), "bad-block"),
    row(
      "bad-max-last-full",
      blockMessage(0xffffffff, 0xfffff, zeros(4096)),
      "bad-block",
    ),
    row("bad-index-past-full", hex(blockHeader(4096, 1)), "bad-block"),
    row(
      "bad-signal-interface-0",
      hex(
        "0b 01 00000000 05000000 ab89674523010000 0000000000000000 02100000 01000000 abcd",
      ),
      "zero-number",
    ),
    row(
      "bad-cut-33-kind-6",
      hex(
        "0b 06 01000000 05000000 ab89674523010000 0000000000000000 02100000 010000",
      ),
      "too-short",
    ),
  ];
}
