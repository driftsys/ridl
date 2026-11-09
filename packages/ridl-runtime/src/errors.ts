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
 */
export class CatalogMismatch extends Error {
  readonly expected: string;
  readonly actual: string;

  constructor(expected: string, actual: string) {
    super(
      `catalog hash mismatch: the TypeScript expects ${expected}, the wasm module has ${actual}`,
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

/** A locally detected invalid stream payload. */
export class StreamError extends Error {
  readonly failure:
    | { category: "corrupt" }
    | {
      category: "contract";
      contract: { category: "invalid-value"; rule: PayloadRule };
    };

  constructor(failure: StreamError["failure"], options?: ErrorOptions) {
    super(
      failure.category === "contract"
        ? `invalid stream payload: ${failure.contract.rule}`
        : "malformed stream payload",
      options,
    );
    this.name = "StreamError";
    this.failure = failure;
  }
}

/** Thrown when construction of a named scalar breaks a checked constraint. */
export class ConstraintError extends RangeError {
  readonly rule: "range" | "step" | "length" | "pattern";
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
