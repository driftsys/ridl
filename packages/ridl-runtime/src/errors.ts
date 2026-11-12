/** Thrown when a module cannot be loaded or does not follow the codec ABI. */
export class CodecError extends Error {
  /**
   * The name of the export that is missing or has the wrong type, or
   * `ridl_alloc` when the module is out of memory (it returned 0 for a
   * request of more than 0 bytes); undefined otherwise.
   */
  readonly exportName?: string;

  constructor(message: string, exportName?: string) {
    super(message);
    this.name = "CodecError";
    this.exportName = exportName;
  }
}

/**
 * Thrown when the catalog hash of a module differs from the hash that the
 * generated TypeScript expects. Both values are lower-case hexadecimal.
 * `holder` names whose hash differs; it is the wasm module unless a port.
 */
export class CatalogMismatch extends Error {
  readonly expected: string;
  readonly actual: string;

  constructor(
    expected: string,
    actual: string,
    holder: string = "the wasm module",
  ) {
    super(
      `catalog hash mismatch: the TypeScript expects ${expected}, ${holder} has ${actual}`,
    );
    this.name = "CatalogMismatch";
    this.expected = expected;
    this.actual = actual;
  }
}

/**
 * Thrown when the wasm module has no payload type of the given name. The name
 * is the one that `ridl_type_id` looks up, for example `cabin.Door`.
 */
export class UnknownPayloadType extends Error {
  readonly type: string;

  constructor(type: string) {
    super(`the wasm module has no payload type "${type}"`);
    this.name = "UnknownPayloadType";
    this.type = type;
  }
}

/** The text of a payload rule reported by the generated codecs. */
export type PayloadRule =
  | "length"
  | "pattern"
  | "range"
  | "step"
  | "unique"
  | "utf8"
  | "variant";

/** A contract error carried by the answer of a provider. */
export type Contract =
  | { category: "invalid-value"; rule: PayloadRule }
  | { category: "precondition-failed" }
  | { category: "contract-broken" }
  | { category: "unknown-interaction" };

/** Why a call, a read or a subscription did not complete. */
export type CallFailure =
  | { category: "contract"; contract: Contract }
  | { category: "corrupt" }
  | { category: "busy" }
  | { category: "undelivered" }
  | { category: "timeout" }
  | { category: "down" }
  | { category: "interrupted" }
  | { category: "unsupported" };

/** The error that a port rejects or throws when a call does not complete. */
export class CallError extends Error {
  /** The category of the failure, with its contract when there is one. */
  readonly failure: CallFailure;

  constructor(failure: CallFailure, options?: ErrorOptions) {
    super(
      failure.category === "contract"
        ? `call failed: contract, ${failure.contract.category}`
        : `call failed: ${failure.category}`,
      options,
    );
    this.name = "CallError";
    this.failure = failure;
  }
}

/**
 * A locally detected invalid stream payload. This is `CallError`: a stream
 * failure has the category `contract` with an `invalid-value` rule, or
 * `corrupt`, which are two of the categories of a call failure.
 *
 * @deprecated Use `CallError`.
 */
export const StreamError = CallError;
/** @deprecated Use `CallError`. */
export type StreamError = CallError;

/** Thrown when construction of a named scalar breaks a checked constraint. */
export class ConstraintError extends RangeError {
  readonly rule: "range" | "step" | "length" | "pattern" | "utf8";
  readonly type: string;

  constructor(
    rule: ConstraintError["rule"],
    type: string,
    value: unknown,
    constraint: string,
  ) {
    let text: string;
    if (value instanceof Uint8Array) {
      text = `<${value.length} bytes>`;
    } else if (typeof value === "string") {
      text = "";
      let count = 0;
      for (const point of value) {
        if (count === 64) {
          text += "…";
          break;
        }
        text += point;
        count++;
      }
    } else {
      text = String(value);
    }
    super(`${type}: ${text} breaks the ${rule} rule ${constraint}`);
    this.name = "ConstraintError";
    this.rule = rule;
    this.type = type;
  }
}
