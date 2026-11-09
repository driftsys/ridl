/**
 * The number of Unicode code points in `value`. A code point outside the Basic
 * Multilingual Plane counts once, where `value.length` counts its two UTF-16
 * code units. An unpaired surrogate counts once.
 *
 * This is the length that a string constraint measures (typl §5.3), as
 * `chars()` is in the Rust backend.
 */
export function codePointLength(value: string): number {
  let count = 0;
  for (const _ of value) {
    count++;
  }
  return count;
}
