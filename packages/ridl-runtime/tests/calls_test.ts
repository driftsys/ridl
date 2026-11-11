import {
  assert,
  assertEquals,
  assertInstanceOf,
  assertThrows,
} from "@std/assert";
import {
  type CallError,
  CatalogMismatch,
  checkCatalog,
  CodecError,
  decodeReply,
  encodeRequest,
} from "../mod.ts";
import type { Codec, PayloadCodec, VerifyResult } from "../mod.ts";

const hash = Uint8Array.from({ length: 32 }, (_, i) => i);
const other = Uint8Array.from({ length: 32 }, (_, i) => 255 - i);

const codecWith = (
  result: VerifyResult,
  catalogHash: Uint8Array = hash,
): Codec => ({
  catalogHash,
  typeId: () => 0,
  verify: () => result,
});

const bytesCodec = (
  overrides: Partial<PayloadCodec<number>> = {},
): PayloadCodec<number> => ({
  type: "t.N",
  encode: (n) => Uint8Array.of(n),
  decode: (b) => b[0],
  ...overrides,
});

Deno.test("check_catalog_names_the_holder_of_the_differing_hash", () => {
  checkCatalog(hash, { catalogHash: hash }, codecWith({ ok: true }));
  const port = assertThrows(() =>
    checkCatalog(hash, { catalogHash: other }, codecWith({ ok: true }))
  );
  assertInstanceOf(port, CatalogMismatch);
  assert(port.message.includes("the port has"));
  const wasm = assertThrows(() =>
    checkCatalog(hash, { catalogHash: hash }, codecWith({ ok: true }, other))
  );
  assertInstanceOf(wasm, CatalogMismatch);
  assert(wasm.message.includes("the wasm module has"));
  assertThrows(
    () =>
      checkCatalog(
        hash,
        { catalogHash: hash.slice(1) },
        codecWith({ ok: true }),
      ),
    CatalogMismatch,
  );
});

Deno.test("encode_request_maps_a_failed_verification_to_a_call_error", () => {
  assertEquals(
    encodeRequest(codecWith({ ok: true }), bytesCodec(), 0, 7),
    Uint8Array.of(7),
  );
  const rule = assertThrows(() =>
    encodeRequest(
      codecWith({ ok: false, kind: "rule", code: 4, rule: "range" }),
      bytesCodec(),
      0,
      7,
    )
  ) as CallError;
  assertEquals(rule.failure, {
    category: "contract",
    contract: { category: "invalid-value", rule: "range" },
  });
  const malformed = assertThrows(() =>
    encodeRequest(
      codecWith({ ok: false, kind: "malformed", code: 1 }),
      bytesCodec(),
      0,
      7,
    )
  ) as CallError;
  assertEquals(malformed.failure, { category: "corrupt" });
  assertThrows(
    () =>
      encodeRequest(
        codecWith({ ok: false, kind: "rule", code: 99, rule: "nope" }),
        bytesCodec(),
        0,
        7,
      ),
    CodecError,
  );
  assertThrows(
    () =>
      encodeRequest(
        codecWith({ ok: false, kind: "unknown-type", code: -1 }),
        bytesCodec(),
        0,
        7,
      ),
    CodecError,
  );
});

Deno.test("encode_request_maps_an_encoder_failure", () => {
  const ruleFailure = Object.assign(new RangeError("x"), {
    name: "RuleError",
    rule: "variant",
  });
  const refused = assertThrows(() =>
    encodeRequest(
      codecWith({ ok: true }),
      bytesCodec({
        encode: () => {
          throw ruleFailure;
        },
      }),
      0,
      1,
    )
  ) as CallError;
  assertEquals(refused.failure, {
    category: "contract",
    contract: { category: "invalid-value", rule: "variant" },
  });
  const other = assertThrows(() =>
    encodeRequest(
      codecWith({ ok: true }),
      bytesCodec({
        encode: () => {
          throw new TypeError("x");
        },
      }),
      0,
      1,
    )
  ) as CallError;
  assertEquals(other.failure, { category: "corrupt" });
});

Deno.test("decode_reply_verifies_before_it_decodes", () => {
  assertEquals(
    decodeReply(codecWith({ ok: true }), bytesCodec(), 0, Uint8Array.of(5)),
    5,
  );
  let decoded = false;
  assertThrows(() =>
    decodeReply(
      codecWith({ ok: false, kind: "rule", code: 3, rule: "length" }),
      bytesCodec({
        decode: () => {
          decoded = true;
          return 0;
        },
      }),
      0,
      Uint8Array.of(5),
    )
  );
  assertEquals(decoded, false);
  const failed = assertThrows(() =>
    decodeReply(
      codecWith({ ok: true }),
      bytesCodec({
        decode: () => {
          throw new RangeError("x");
        },
      }),
      0,
      Uint8Array.of(5),
    )
  ) as CallError;
  assertEquals(failed.failure, { category: "corrupt" });
});
