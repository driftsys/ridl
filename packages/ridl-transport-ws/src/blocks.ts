/**
 * The constants and arithmetic of `block` that the frame codec, the
 * reassembly and the session share. This module is internal: the package
 * entry point does not export it, except `BLOCK_BYTES` through `frame.ts`.
 */

/** The bytes of payload in every `block` but the last (binding version 2). */
export const BLOCK_BYTES = 4_096;

/** The form code of `block`, which frame version 1 does not have. */
export const FORM_BLOCK = 11;

/** The number of blocks of a payload of `total` bytes; `total` is at least 1. */
export function blockCount(total: number): number {
  return Math.floor((total - 1) / BLOCK_BYTES) + 1;
}
