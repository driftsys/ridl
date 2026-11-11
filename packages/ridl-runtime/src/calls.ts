import type { Codec, PayloadCodec, VerifyResult } from "./codec.ts";
import {
  CallError,
  CatalogMismatch,
  CodecError,
  type PayloadRule,
} from "./errors.ts";
import type { Attached } from "./ports.ts";

const RULES: readonly string[] = [
  "length",
  "pattern",
  "range",
  "step",
  "unique",
  "utf8",
  "variant",
];

const hex = (hash: Uint8Array) =>
  Array.from(hash, (b) => b.toString(16).padStart(2, "0")).join("");

const sameHash = (a: Uint8Array, b: Uint8Array) =>
  a.length === b.length && a.every((byte, i) => byte === b[i]);

/**
 * Throws `CatalogMismatch` when the catalog hash of the port or of the codec
 * differs from the hash the generated module expects.
 */
export function checkCatalog(
  expected: Uint8Array,
  port: Attached,
  codec: Codec,
): void {
  if (!sameHash(expected, port.catalogHash)) {
    throw new CatalogMismatch(
      hex(expected),
      hex(port.catalogHash),
      "the port",
    );
  }
  if (!sameHash(expected, codec.catalogHash)) {
    throw new CatalogMismatch(hex(expected), hex(codec.catalogHash));
  }
}

function ruleError(rule: PayloadRule, cause?: unknown): CallError {
  return new CallError(
    { category: "contract", contract: { category: "invalid-value", rule } },
    cause === undefined ? undefined : { cause },
  );
}

/**
 * Throws what a failed verification of one payload means for a call: a
 * `CallError` of the category `contract` for a failed rule, `corrupt` for a
 * malformed payload, and a `CodecError` for a rule text or a type the generated
 * module does not know.
 */
function assertVerified(result: VerifyResult): void {
  if (result.ok) return;
  if (result.kind === "malformed") throw new CallError({ category: "corrupt" });
  if (result.kind === "rule") {
    if (result.rule !== undefined && RULES.includes(result.rule)) {
      throw ruleError(result.rule as PayloadRule);
    }
    throw new CodecError(`unknown payload rule: ${result.rule}`);
  }
  throw new CodecError("unknown payload type during verification");
}

function isRuleException(error: unknown, names: readonly string[]): boolean {
  return error instanceof Error && names.includes(error.name) &&
    "rule" in error && typeof error.rule === "string" &&
    RULES.includes(error.rule);
}

/**
 * Encodes the request of a call and verifies the bytes with the codec before
 * they are sent. A value that the encoder refuses for a rule, and bytes that
 * fail a rule, throw a `CallError` of the category `contract`; other encoder
 * failures and malformed bytes throw `corrupt`.
 */
export function encodeRequest<T>(
  codec: Codec,
  payload: PayloadCodec<T>,
  typeId: number,
  value: T,
): Uint8Array {
  let bytes: Uint8Array;
  try {
    bytes = payload.encode(value);
  } catch (error) {
    if (isRuleException(error, ["RuleError", "ConstraintError"])) {
      throw ruleError(
        (error as { rule: PayloadRule }).rule,
        error,
      );
    }
    throw new CallError({ category: "corrupt" }, { cause: error });
  }
  assertVerified(codec.verify(typeId, bytes));
  return bytes;
}

/**
 * Verifies the reply bytes of a call with the codec and decodes them. Failures
 * map as in `encodeRequest`, so a call and a stream element fail the same way.
 */
export function decodeReply<T>(
  codec: Codec,
  payload: PayloadCodec<T>,
  typeId: number,
  bytes: Uint8Array,
): T {
  assertVerified(codec.verify(typeId, bytes));
  try {
    return payload.decode(bytes);
  } catch (error) {
    if (isRuleException(error, ["RuleError"])) {
      throw ruleError((error as { rule: PayloadRule }).rule, error);
    }
    throw new CallError({ category: "corrupt" }, { cause: error });
  }
}
