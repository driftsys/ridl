/**
 * The reassembly of the `block` frames of a `fixed` (binding version 2,
 * frame specification §11.1).
 *
 * A session holds one {@link Reassembly}. It receives each decoded block,
 * whose single-block rules the codec has already checked, and checks the
 * sequence: the blocks of one payload arrive in order, from index 0, with the
 * same identity, envelope and `total`, and the blocks of two payloads do not
 * interleave. A payload whose `total` is above the member's limit is refused
 * at its first block, before a byte is copied, and its other blocks are
 * skipped with their sequence still checked.
 *
 * @module
 */

import { BLOCK_BYTES, blockCount } from "./blocks.ts";
import { FIXED_CEILING, Kind } from "./frame.ts";
import type { Frame, Target } from "./frame.ts";

/** A decoded `block` frame. */
export type BlockFrame = Extract<Frame, { form: "block" }>;

/** What one block did to the slot. */
export type BlockStep =
  /** The block was copied; more blocks follow. */
  | { step: "collecting" }
  /** The last block was copied; the slot is empty. */
  | { step: "complete"; payload: Uint8Array }
  /** An index 0 whose `total` is above the limit; nothing was copied. */
  | { step: "over-limit" }
  /** A block of a refused payload, whose sequence was checked. */
  | { step: "skipping" }
  /** A sequence error; the slot is empty. */
  | { step: "sequence" };

/**
 * A `fixed`'s limit held to `FIXED_CEILING`: a `limit` above 65,280 counts
 * as 65,280, and one that is not a finite number counts as 0.
 */
export function fixedLimit(limit: number): number {
  if (!Number.isFinite(limit)) return 0;
  return Math.min(limit, FIXED_CEILING);
}

/** The payload in progress. */
interface Slot {
  iface: number;
  ord: number;
  stamp: bigint;
  seq: bigint;
  total: number;
  /** The number of blocks of the payload. */
  count: number;
  /** The index the next block must have. */
  next: number;
  /** The bytes copied so far, or `null` when the payload is skipped. */
  buffer: Uint8Array | null;
}

/**
 * The one reassembly of a session. A payload goes into a buffer of `total`
 * bytes that is allocated for it and handed to the caller when the payload
 * completes; the slot never writes to that buffer again.
 */
export class Reassembly {
  #slot: Slot | null = null;

  /**
   * Applies one decoded block. `limit` is the member's limit as the
   * session's `#limitOf` gives it, already held by `fixedLimit`; it is read
   * at index 0 only.
   */
  accept(block: BlockFrame, limit: number): BlockStep {
    const slot = this.#slot;
    if (slot === null) return this.#start(block, limit);
    if (
      block.index !== slot.next ||
      block.target.iface !== slot.iface ||
      block.target.ord !== slot.ord ||
      block.envelope.stamp !== slot.stamp ||
      block.envelope.seq !== slot.seq ||
      block.total !== slot.total
    ) {
      this.#slot = null;
      return { step: "sequence" };
    }
    slot.buffer?.set(block.payload, BLOCK_BYTES * block.index);
    slot.next += 1;
    if (slot.next < slot.count) {
      return { step: slot.buffer ? "collecting" : "skipping" };
    }
    this.#slot = null;
    return slot.buffer
      ? { step: "complete", payload: slot.buffer }
      : { step: "skipping" };
  }

  /**
   * Whether a payload of `target` is being collected or skipped: its kind is
   * `Fixed`, and its `iface` and `ord` are the slot's.
   */
  holds(target: Target): boolean {
    const slot = this.#slot;
    return slot !== null && target.kind === Kind.Fixed &&
      target.iface === slot.iface && target.ord === slot.ord;
  }

  /** Drops the payload in progress. */
  clear(): void {
    this.#slot = null;
  }

  /** A block with no payload in progress. */
  #start(block: BlockFrame, limit: number): BlockStep {
    if (block.index !== 0) return { step: "sequence" };
    const count = blockCount(block.total);
    const over = block.total > limit;
    let buffer: Uint8Array | null = null;
    if (!over) {
      buffer = new Uint8Array(block.total);
      buffer.set(block.payload, 0);
      if (count === 1) return { step: "complete", payload: buffer };
    }
    if (count > 1) {
      this.#slot = {
        iface: block.target.iface,
        ord: block.target.ord,
        stamp: block.envelope.stamp,
        seq: block.envelope.seq,
        total: block.total,
        count,
        next: 1,
        buffer,
      };
    }
    return over ? { step: "over-limit" } : { step: "collecting" };
  }
}
