/**
 * Whether `value` holds only Unicode scalar values: every high surrogate is
 * followed by a low surrogate and every low surrogate follows a high one. A
 * JavaScript string may hold an unpaired surrogate, which `TextEncoder`
 * replaces with U+FFFD without an error (typl §4.4).
 *
 * Uses `String.prototype.isWellFormed` where the runtime has it and a scan of
 * the UTF-16 code units elsewhere.
 */
export function isWellFormedString(value: string): boolean {
  const native = (value as { isWellFormed?: () => boolean }).isWellFormed;
  if (typeof native === "function") {
    return native.call(value);
  }
  for (let i = 0; i < value.length; i++) {
    const unit = value.charCodeAt(i);
    if (unit >= 0xd800 && unit <= 0xdbff) {
      const next = value.charCodeAt(i + 1);
      if (!(next >= 0xdc00 && next <= 0xdfff)) return false;
      i++;
    } else if (unit >= 0xdc00 && unit <= 0xdfff) {
      return false;
    }
  }
  return true;
}
