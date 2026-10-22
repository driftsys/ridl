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
