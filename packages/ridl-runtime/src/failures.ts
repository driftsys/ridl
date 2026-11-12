import type { VerifyResult } from "./codec.ts";
import { CallError, CodecError, type PayloadRule } from "./errors.ts";

const RULES: readonly string[] = [
  "length",
  "pattern",
  "range",
  "step",
  "unique",
  "utf8",
  "variant",
];

/** The `CallError` of a payload that fails the rule `rule`. */
export function ruleError(rule: PayloadRule, cause?: unknown): CallError {
  return new CallError(
    { category: "contract", contract: { category: "invalid-value", rule } },
    cause === undefined ? undefined : { cause },
  );
}

/**
 * What a failed verification of one payload means: a `CallError` of the
 * category `contract` for a failed rule, `corrupt` for a malformed payload,
 * and a `CodecError` for a rule text or a type the generated module does not
 * know. Calls, signals, events and stream elements all map through this one
 * function, so a payload fails the same way wherever it is read.
 */
export function verificationFailure(
  result: Exclude<VerifyResult, { ok: true }>,
): CallError | CodecError {
  if (result.kind === "malformed") {
    return new CallError({ category: "corrupt" });
  }
  if (result.kind === "rule") {
    if (result.rule !== undefined && RULES.includes(result.rule)) {
      return ruleError(result.rule as PayloadRule);
    }
    return new CodecError(`unknown payload rule: ${result.rule}`);
  }
  return new CodecError("unknown payload type during verification");
}

/** Throws the failure of a verification that did not pass. */
export function assertVerified(result: VerifyResult): void {
  if (!result.ok) throw verificationFailure(result);
}

/**
 * What an exception of an encoder or a decoder means. A generated module's
 * `RuleError` and the runtime's `ConstraintError` that name a payload rule
 * are a `contract` failure; any other exception is `corrupt`. The exception is
 * kept as the cause.
 */
export function exceptionFailure(error: unknown): CallError {
  if (
    error instanceof Error &&
    (error.name === "RuleError" || error.name === "ConstraintError") &&
    "rule" in error && typeof error.rule === "string" &&
    RULES.includes(error.rule)
  ) {
    return ruleError(error.rule as PayloadRule, error);
  }
  return new CallError({ category: "corrupt" }, { cause: error });
}
