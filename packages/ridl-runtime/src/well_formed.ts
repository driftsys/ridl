import { ConstraintError } from "./errors.ts";

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

/**
 * Returns `value` when it is well-formed and throws a `ConstraintError` with
 * rule `utf8` otherwise. `type` names where the string sits: the qualified name
 * of a payload type and the path of the field, such as `veh.cabin.Door.label`.
 * The generated `encode<Name>` calls it for every string it writes.
 */
export function checkWellFormed(value: string, type: string): string {
  if (!isWellFormedString(value)) {
    throw new ConstraintError("utf8", type, value, "well-formed text");
  }
  return value;
}
