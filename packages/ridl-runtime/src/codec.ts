import { CatalogMismatch, CodecError } from "./errors.ts";

/** The outcome of `Codec.verify`. */
export type VerifyResult =
  | { ok: true }
  | {
    ok: false;
    kind: "malformed" | "rule" | "unknown-type";
    /** The value `ridl_verify` returned: 1, a rule code of 2 or more, or -1. */
    code: number;
    /** The text of the failed rule, when `kind` is `rule` and text was given. */
    rule?: string;
  };

/** A loaded wasm codec module. */
export interface Codec {
  /** The 32-byte catalog hash that the module reports. */
  readonly catalogHash: Uint8Array;
  /** Verifies the encoded payload of the type with the given `type_id`. */
  verify(typeId: number, bytes: Uint8Array): VerifyResult;
}

/** Where the wasm module comes from. */
export type CodecSource =
  | URL
  | Response
  | Promise<Response>
  | ArrayBuffer
  | ArrayBufferView;

/** Imports for the module, in the shape `WebAssembly.instantiate` takes. */
export type CodecImports = Record<string, Record<string, unknown>>;

export interface LoadOptions {
  /** The 32-byte catalog hash that the generated TypeScript expects. */
  catalogHash: Uint8Array;
  /** The text of rule code `n` is at index `n - 2`. */
  ruleText?: readonly string[];
  /** Imports for the module. The ABI needs none. */
  imports?: CodecImports;
}

const HASH_LENGTH = 32;

function hex(bytes: Uint8Array): string {
  return Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
}

function isResponse(value: unknown): value is Response {
  return typeof Response !== "undefined" && value instanceof Response;
}

async function instantiate(
  source: CodecSource,
  imports: WebAssembly.Imports,
): Promise<WebAssembly.Instance> {
  let input: Response | ArrayBuffer | ArrayBufferView;
  if (source instanceof URL) {
    const denoHost =
      (globalThis as { Deno?: { readFile(u: URL): Promise<Uint8Array> } })
        .Deno;
    if (source.protocol === "file:" && denoHost) {
      input = (await denoHost.readFile(source)) as Uint8Array<ArrayBuffer>;
    } else {
      input = await fetch(source);
    }
  } else {
    input = await source;
  }
  if (isResponse(input)) {
    if (!input.ok) {
      throw new CodecError(
        `the wasm module could not be fetched: HTTP ${input.status}`,
      );
    }
    const type = input.headers.get("content-type") ?? "";
    if (
      typeof WebAssembly.instantiateStreaming === "function" &&
      type.split(";")[0].trim().toLowerCase() === "application/wasm"
    ) {
      return (await WebAssembly.instantiateStreaming(input, imports)).instance;
    }
    const bytes = await input.arrayBuffer();
    return (await WebAssembly.instantiate(bytes, imports)).instance;
  }
  return (await WebAssembly.instantiate(input as BufferSource, imports))
    .instance;
}

function fn(
  exports: WebAssembly.Exports,
  name: string,
  // deno-lint-ignore ban-types
): Function {
  const value = exports[name];
  if (typeof value !== "function") {
    throw new CodecError(
      `the wasm module has no function export "${name}"`,
      name,
    );
  }
  return value;
}

/** The error for an allocation that the module reports as failed (pointer 0). */
function outOfMemory(): CodecError {
  return new CodecError(
    "ridl_alloc returned 0: the wasm module is out of memory",
    "ridl_alloc",
  );
}

/** Loads a wasm codec module and checks its catalog hash. */
export async function loadCodec(
  source: CodecSource,
  options: LoadOptions,
): Promise<Codec> {
  if (options.catalogHash.length !== HASH_LENGTH) {
    throw new CodecError(
      `catalogHash must be ${HASH_LENGTH} bytes, got ${options.catalogHash.length}`,
    );
  }
  let instance: WebAssembly.Instance;
  try {
    instance = await instantiate(
      source,
      (options.imports ?? {}) as WebAssembly.Imports,
    );
  } catch (error) {
    if (error instanceof CodecError) throw error;
    throw new CodecError(
      `the wasm module could not be loaded: ${
        error instanceof Error ? error.message : String(error)
      }`,
    );
  }
  const exports = instance.exports;
  const memory = exports.memory;
  if (!(memory instanceof WebAssembly.Memory)) {
    throw new CodecError(
      'the wasm module has no memory export "memory"',
      "memory",
    );
  }
  const alloc = fn(exports, "ridl_alloc") as (len: number) => number;
  const free = fn(exports, "ridl_free") as (ptr: number, len: number) => void;
  const catalogHash = fn(exports, "ridl_catalog_hash") as (
    out: number,
  ) => void;
  const verify = fn(exports, "ridl_verify") as (
    typeId: number,
    ptr: number,
    len: number,
  ) => number;

  // The hash is read through a buffer that the module allocates.
  const hashPtr = alloc(HASH_LENGTH) >>> 0;
  if (hashPtr === 0) throw outOfMemory();
  let actual: Uint8Array;
  try {
    catalogHash(hashPtr);
    actual = new Uint8Array(memory.buffer, hashPtr, HASH_LENGTH).slice();
  } finally {
    free(hashPtr, HASH_LENGTH);
  }
  const expected = options.catalogHash;
  if (hex(actual) !== hex(expected)) {
    throw new CatalogMismatch(hex(expected), hex(actual));
  }

  const ruleText = options.ruleText;
  return {
    catalogHash: actual,
    verify(typeId: number, bytes: Uint8Array): VerifyResult {
      if (!Number.isInteger(typeId) || typeId < 0 || typeId > 0xffffffff) {
        throw new CodecError(
          `typeId must be an integer in [0, 2^32), got ${String(typeId)}`,
        );
      }
      const ptr = alloc(bytes.length) >>> 0;
      // A zero length may legitimately return 0; any other 0 is a failure.
      if (ptr === 0 && bytes.length > 0) throw outOfMemory();
      let code: number;
      try {
        // The allocation may have grown the memory, so the buffer is read
        // again here and not kept from an earlier call.
        new Uint8Array(memory.buffer, ptr, bytes.length).set(bytes);
        code = verify(typeId, ptr, bytes.length);
      } finally {
        free(ptr, bytes.length);
      }
      if (code === 0) return { ok: true };
      if (code === 1) return { ok: false, kind: "malformed", code };
      if (code === -1) return { ok: false, kind: "unknown-type", code };
      if (code < -1) {
        throw new CodecError(`ridl_verify returned the invalid code ${code}`);
      }
      const result: VerifyResult = { ok: false, kind: "rule", code };
      const text = ruleText?.[code - 2];
      if (text !== undefined) result.rule = text;
      return result;
    },
  };
}
