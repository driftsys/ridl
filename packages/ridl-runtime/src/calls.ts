import type { Codec, PayloadCodec, VerifyResult } from "./codec.ts";
import {
  CallError,
  CatalogMismatch,
  CodecError,
  type PayloadRule,
} from "./errors.ts";
import type { Detection } from "./checks.ts";
import type {
  Attached,
  Occurrence,
  RawOccurrence,
  RawSample,
  Sample,
} from "./ports.ts";

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

/** Decodes one payload of a sample, or says what the check detected. */
function tryDecode<T>(
  codec: Codec,
  payload: PayloadCodec<T>,
  typeId: number,
  bytes: Uint8Array,
): { value: T } | { detected: Detection } {
  try {
    return { value: decodeReply(codec, payload, typeId, bytes) };
  } catch (error) {
    if (error instanceof CallError) {
      const failure = error.failure;
      if (failure.category === "contract") {
        const contract = failure.contract;
        if (contract.category === "invalid-value") {
          return { detected: { rule: contract.rule } };
        }
      }
      if (failure.category === "corrupt") return { detected: "corrupt" };
    }
    throw error;
  }
}

/**
 * The sample of a signal that has not been published: `init` provenance, a
 * zero envelope and the init value of the type.
 */
export function initSample<T>(init: () => T): Sample<T> {
  return {
    provenance: "init",
    envelope: { stamp: 0n, seq: 0n },
    value: init(),
  };
}

/**
 * Verifies and decodes the payload of a raw sample. The value of a live sample
 * is its payload. Under `init` and `invalid`, and for a live payload that
 * fails verification (the sample then becomes `invalid` with `detected` set),
 * the value is the last good payload when it still verifies, and `init()`
 * otherwise, so the value is never absent. A rule text the generated module
 * does not know throws a `CodecError`.
 */
export function decodeSample<T>(
  codec: Codec,
  payload: PayloadCodec<T>,
  typeId: number,
  raw: RawSample,
  init: () => T,
): Sample<T> {
  const fallback = (): T => {
    if (raw.lastGood === null) return init();
    const last = tryDecode(codec, payload, typeId, raw.lastGood);
    return "value" in last ? last.value : init();
  };
  if (raw.payload !== null) {
    const live = tryDecode(codec, payload, typeId, raw.payload);
    if ("value" in live) {
      // A port sets `detected` only on a sample without a payload.
      return {
        provenance: raw.provenance,
        envelope: raw.envelope,
        value: live.value,
      };
    }
    return {
      provenance: "invalid",
      envelope: raw.envelope,
      value: fallback(),
      detected: live.detected,
    };
  }
  return raw.detected === undefined
    ? { provenance: raw.provenance, envelope: raw.envelope, value: fallback() }
    : {
      provenance: raw.provenance,
      envelope: raw.envelope,
      value: fallback(),
      detected: raw.detected,
    };
}

/**
 * Verifies and decodes the payload of a raw occurrence. An occurrence that the
 * port refused, and one whose payload fails verification, is returned with
 * `detected` set to `corrupt` and no value. A rule text the generated module
 * does not know throws a `CodecError`.
 */
export function decodeOccurrence<T>(
  codec: Codec,
  payload: PayloadCodec<T>,
  typeId: number,
  raw: RawOccurrence,
): Occurrence<T> {
  const refused = (): Occurrence<T> => ({
    iface: raw.iface,
    ord: raw.ord,
    envelope: raw.envelope,
    detected: "corrupt",
  });
  if (raw.detected !== undefined) return refused();
  const decoded = tryDecode(codec, payload, typeId, raw.payload);
  if (!("value" in decoded)) return refused();
  return {
    iface: raw.iface,
    ord: raw.ord,
    envelope: raw.envelope,
    value: decoded.value,
  };
}
