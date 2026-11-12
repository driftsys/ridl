import {
  assert,
  assertEquals,
  assertInstanceOf,
  assertThrows,
} from "@std/assert";
import {
  CallError,
  CatalogMismatch,
  checkCatalog,
  CodecError,
  ConstraintError,
  decodeOccurrence,
  decodeReply,
  decodeSample,
  decodeStream,
  encodeRequest,
  initSample,
  StreamError,
} from "../mod.ts";
import type {
  Codec,
  PayloadCodec,
  RawOccurrence,
  RawSample,
  VerifyResult,
} from "../mod.ts";

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

const toHex = (bytes: Uint8Array) =>
  [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");

Deno.test("check_catalog_reports_the_expected_and_the_actual_hash", () => {
  const port = assertThrows(() =>
    checkCatalog(hash, { catalogHash: other }, codecWith({ ok: true }))
  );
  assertInstanceOf(port, CatalogMismatch);
  assertEquals(port.expected, toHex(hash));
  assertEquals(port.actual, toHex(other));
  const wasm = assertThrows(() =>
    checkCatalog(hash, { catalogHash: hash }, codecWith({ ok: true }, other))
  );
  assertInstanceOf(wasm, CatalogMismatch);
  assertEquals(wasm.expected, toHex(hash));
  assertEquals(wasm.actual, toHex(other));
});

Deno.test("a_call_error_message_names_the_category_and_the_contract_category", () => {
  assertEquals(
    new CallError({ category: "timeout" }).message,
    "call failed: timeout",
  );
  assertEquals(
    new CallError({
      category: "contract",
      contract: { category: "invalid-value", rule: "range" },
    }).message,
    "call failed: contract, invalid-value",
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

Deno.test("decode_reply_maps_a_rule_error_of_the_decoder_to_a_contract_error", () => {
  const ruleFailure = Object.assign(new RangeError("x"), {
    name: "RuleError",
    rule: "range",
  });
  const failed = assertThrows(() =>
    decodeReply(
      codecWith({ ok: true }),
      bytesCodec({
        decode: () => {
          throw ruleFailure;
        },
      }),
      0,
      Uint8Array.of(5),
    )
  ) as CallError;
  assertEquals(failed.failure, {
    category: "contract",
    contract: { category: "invalid-value", rule: "range" },
  });
  assertEquals(failed.cause, ruleFailure);
});

Deno.test("encode_request_maps_a_constraint_error_of_the_encoder", () => {
  const refused = new ConstraintError("length", "t.N", "abc", "[0..2]");
  const failed = assertThrows(() =>
    encodeRequest(
      codecWith({ ok: true }),
      bytesCodec({
        encode: () => {
          throw refused;
        },
      }),
      0,
      1,
    )
  ) as CallError;
  assertEquals(failed.failure, {
    category: "contract",
    contract: { category: "invalid-value", rule: "length" },
  });
  assertEquals(failed.cause, refused);
});

Deno.test("an_encoder_failure_that_is_not_a_rule_keeps_its_cause", () => {
  const cause = new TypeError("x");
  const failed = assertThrows(() =>
    encodeRequest(
      codecWith({ ok: true }),
      bytesCodec({
        encode: () => {
          throw cause;
        },
      }),
      0,
      1,
    )
  ) as CallError;
  assertEquals(failed.cause, cause);
});

Deno.test("check_catalog_refuses_a_longer_hash_with_a_matching_prefix", () => {
  const longer = Uint8Array.from([...hash, 0]);
  assertThrows(
    () => checkCatalog(hash, { catalogHash: longer }, codecWith({ ok: true })),
    CatalogMismatch,
  );
  assertThrows(
    () =>
      checkCatalog(
        hash,
        { catalogHash: hash },
        codecWith({ ok: true }, longer),
      ),
    CatalogMismatch,
  );
});

const envelope = { stamp: 5n, seq: 6n };
const rawSample = (overrides: Partial<RawSample>): RawSample => ({
  provenance: "live",
  envelope,
  payload: null,
  lastGood: null,
  ...overrides,
});
/** Fails verification of the bytes that start with 9, and of no others. */
const rejectNine: Codec = {
  catalogHash: hash,
  typeId: () => 0,
  verify: (_, bytes) =>
    bytes[0] === 9 ? { ok: false, kind: "rule", code: 4, rule: "range" } : {
      ok: true,
    },
};

Deno.test("init_sample_holds_the_init_value_under_a_zero_envelope", () => {
  assertEquals(initSample(() => 7), {
    provenance: "init",
    envelope: { stamp: 0n, seq: 0n },
    value: 7,
  });
});

Deno.test("decode_sample_decodes_a_live_payload", () => {
  const sample = decodeSample(
    rejectNine,
    bytesCodec(),
    0,
    rawSample({ payload: Uint8Array.of(3) }),
    () => 0,
  );
  assertEquals(sample, { provenance: "live", envelope, value: 3 });
});

Deno.test("decode_sample_falls_back_to_the_last_good_value_then_to_init", () => {
  const invalid = (lastGood: Uint8Array | null) =>
    decodeSample(
      rejectNine,
      bytesCodec(),
      0,
      rawSample({ provenance: "invalid", lastGood, detected: "corrupt" }),
      () => 100,
    );
  assertEquals(invalid(Uint8Array.of(4)), {
    provenance: "invalid",
    envelope,
    value: 4,
    detected: "corrupt",
  });
  assertEquals(invalid(null).value, 100);
  // A last good payload that fails verification is not trusted.
  assertEquals(invalid(Uint8Array.of(9)).value, 100);
  const init = decodeSample(
    rejectNine,
    bytesCodec(),
    0,
    rawSample({ provenance: "init", lastGood: Uint8Array.of(4) }),
    () => 100,
  );
  assertEquals(init, { provenance: "init", envelope, value: 4 });
});

Deno.test("decode_sample_marks_a_live_payload_that_fails_verification_invalid", () => {
  const sample = decodeSample(
    rejectNine,
    bytesCodec(),
    0,
    rawSample({ payload: Uint8Array.of(9), lastGood: Uint8Array.of(2) }),
    () => 100,
  );
  assertEquals(sample, {
    provenance: "invalid",
    envelope,
    value: 2,
    detected: { rule: "range" },
  });
});

Deno.test("decode_sample_rejects_an_unknown_rule_text", () => {
  const unknown: Codec = {
    ...rejectNine,
    verify: () => ({ ok: false, kind: "rule", code: 9, rule: "mystery" }),
  };
  assertThrows(
    () =>
      decodeSample(
        unknown,
        bytesCodec(),
        0,
        rawSample({ payload: Uint8Array.of(1) }),
        () => 0,
      ),
    CodecError,
  );
});

Deno.test("decode_sample_rejects_an_unknown_type_id", () => {
  const unknown: Codec = {
    ...rejectNine,
    verify: () => ({ ok: false, kind: "unknown-type", code: -1 }),
  };
  assertThrows(
    () =>
      decodeSample(
        unknown,
        bytesCodec(),
        0,
        rawSample({ payload: Uint8Array.of(1) }),
        () => 0,
      ),
    CodecError,
  );
});

Deno.test("decode_occurrence_decodes_and_marks_refusals", () => {
  const raw = (overrides: Partial<RawOccurrence>): RawOccurrence => ({
    iface: 1,
    ord: 2,
    envelope,
    payload: Uint8Array.of(3),
    ...overrides,
  });
  assertEquals(decodeOccurrence(rejectNine, bytesCodec(), 0, raw({})), {
    iface: 1,
    ord: 2,
    envelope,
    value: 3,
  });
  const refused = { iface: 1, ord: 2, envelope, detected: "corrupt" as const };
  assertEquals(
    decodeOccurrence(
      rejectNine,
      bytesCodec(),
      0,
      raw({ payload: new Uint8Array(), detected: "corrupt" }),
    ),
    refused,
  );
  assertEquals(
    decodeOccurrence(
      rejectNine,
      bytesCodec(),
      0,
      raw({ payload: Uint8Array.of(9) }),
    ),
    refused,
  );
});

Deno.test("encode_request_refuses_bytes_longer_than_the_limit_before_verifying", () => {
  let verified = 0;
  const codec: Codec = {
    ...codecWith({ ok: true }),
    verify: () => {
      verified++;
      return { ok: true };
    },
  };
  assertEquals(
    encodeRequest(
      codec,
      bytesCodec({ encode: () => new Uint8Array(4) }),
      0,
      1,
      4,
    )
      .length,
    4,
  );
  assertEquals(verified, 1);
  const refused = assertThrows(() =>
    encodeRequest(
      codec,
      bytesCodec({ encode: () => new Uint8Array(5) }),
      0,
      1,
      4,
    )
  ) as CallError;
  assertEquals(refused.failure, { category: "corrupt" });
  assertEquals(verified, 1);
  // Without a limit, any length goes to the verification.
  encodeRequest(codec, bytesCodec({ encode: () => new Uint8Array(500) }), 0, 1);
  assertEquals(verified, 2);
});

Deno.test("a_failed_verification_maps_the_same_for_a_request_a_reply_and_a_stream_element", async () => {
  const results: VerifyResult[] = [
    { ok: false, kind: "malformed", code: 1 },
    { ok: false, kind: "rule", code: 4, rule: "range" },
    { ok: false, kind: "rule", code: 99, rule: "nope" },
    { ok: false, kind: "unknown-type", code: -1 },
  ];
  const outcome = (error: unknown) =>
    error instanceof CallError
      ? { name: error.name, failure: error.failure }
      : { name: (error as Error).name, message: (error as Error).message };
  for (const result of results) {
    const codec = codecWith(result);
    const request = assertThrows(() =>
      encodeRequest(codec, bytesCodec(), 0, 1)
    );
    const reply = assertThrows(() =>
      decodeReply(codec, bytesCodec(), 0, Uint8Array.of(1))
    );
    const element = await Array.fromAsync(
      ReadableStream.from([Uint8Array.of(1)]).pipeThrough(
        decodeStream((bytes) => bytes[0], () => result),
      ),
    ).then(() => undefined, (error) => error);
    assertEquals(outcome(request), outcome(reply));
    assertEquals(outcome(element), outcome(reply));
  }
});

Deno.test("stream_error_is_the_call_error_class", () => {
  assert(StreamError === CallError);
  const error = new StreamError({ category: "corrupt" });
  assertInstanceOf(error, CallError);
  assertEquals(error.name, "CallError");
});

Deno.test("a_decoder_constraint_error_is_a_contract_failure_for_a_reply_and_a_stream_element", async () => {
  const refused = new ConstraintError("utf8", "t.N", "\ud800", "well-formed");
  const throwing = bytesCodec({
    decode: () => {
      throw refused;
    },
  });
  const reply = assertThrows(() =>
    decodeReply(codecWith({ ok: true }), throwing, 0, Uint8Array.of(1))
  ) as CallError;
  const element = await Array.fromAsync(
    ReadableStream.from([Uint8Array.of(1)]).pipeThrough(
      decodeStream(throwing.decode, () => ({ ok: true })),
    ),
  ).then(() => undefined, (error) => error as CallError);
  for (const error of [reply, element]) {
    assertEquals(error?.failure, {
      category: "contract",
      contract: { category: "invalid-value", rule: "utf8" },
    });
    assertEquals(error?.cause, refused);
  }
});

Deno.test("an_exception_with_a_rule_outside_the_payload_rules_is_corrupt", async () => {
  const strange = Object.assign(new RangeError("x"), {
    name: "RuleError",
    rule: "bogus",
  });
  const thrower = () => {
    throw strange;
  };
  const request = assertThrows(() =>
    encodeRequest(
      codecWith({ ok: true }),
      bytesCodec({ encode: thrower }),
      0,
      1,
    )
  ) as CallError;
  const reply = assertThrows(() =>
    decodeReply(
      codecWith({ ok: true }),
      bytesCodec({ decode: thrower }),
      0,
      Uint8Array.of(1),
    )
  ) as CallError;
  const element = await Array.fromAsync(
    ReadableStream.from([Uint8Array.of(1)]).pipeThrough(
      decodeStream(thrower, () => ({ ok: true })),
    ),
  ).then(() => undefined, (error) => error as CallError);
  for (const error of [request, reply, element]) {
    assertEquals(error?.failure, { category: "corrupt" });
    assertEquals(error?.cause, strange);
  }
});

Deno.test("encode_request_with_a_limit_of_zero_refuses_only_a_non_empty_request", () => {
  assertEquals(
    encodeRequest(
      codecWith({ ok: true }),
      bytesCodec({ encode: () => new Uint8Array(0) }),
      0,
      1,
      0,
    ).length,
    0,
  );
  const refused = assertThrows(() =>
    encodeRequest(codecWith({ ok: true }), bytesCodec(), 0, 1, 0)
  ) as CallError;
  assertEquals(refused.failure, { category: "corrupt" });
});
