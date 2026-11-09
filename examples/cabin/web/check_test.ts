// Tests of the check in check.ts and of the generated codec module it reads.
// Run with `deno test --allow-read check_test.ts` after `just cabin-ws-demo`
// has written generated/, which holds the codec module veh.cabin.codec.ts
// that `ridl build` writes, the `flatc` code it imports, and the wasm codec
// veh_cabin.wasm.

import assert from "node:assert/strict";
import {
  type Codec,
  ConstraintError,
  loadCodec,
  UnknownPayloadType,
  type VerifyResult,
} from "../../../packages/ridl-runtime/mod.ts";
import {
  Cabin,
  CATALOG_HASH,
  decodeTemperature,
  encodeAverage,
  encodeBlock,
  encodeCode,
  encodeDigited,
  encodeGlyph,
  encodeLevel,
  encodeSetpoint,
  encodeTag,
  encodeTemperature,
  encodeTenth,
  encodeWindow,
  Horn,
  PAYLOAD_TYPES,
  RULES,
  signalChecks,
} from "./generated/veh.cabin.codec.ts";
import {
  makeAverage,
  makeBlock,
  makeCode,
  makeDigited,
  makeGlyph,
  makeLevel,
  makeSetpoint,
  makeTag,
  makeTemperature,
  makeTenth,
  makeWindow,
} from "./generated/veh.cabin.ts";
import type {
  Average,
  Block,
  Code,
  Digited,
  Glyph,
  Level,
  Setpoint,
  Tag,
  Temperature,
  Tenth,
  Window,
} from "./generated/veh.cabin.ts";
import {
  isPass,
  RANGE_RULE,
  selfTest,
  TemperatureCheck,
  verifyPayload,
} from "./check.ts";

/** The generated `encodeTemperature` of a plain number. */
const encode = (value: number) => encodeTemperature(value as Temperature);

Deno.test("a full lap passes", () => {
  const check = new TemperatureCheck();
  for (let value = -40; value <= 85; value++) {
    assert.deepEqual(check.accept(value), { kind: "continue" });
  }
  assert.deepEqual(check.accept(-40), { kind: "pass" });
  assert.equal(check.progress.wraps, 1);
  assert.equal(check.progress.received, 127);
});

Deno.test("a value outside the range fails", () => {
  const verdict = new TemperatureCheck().accept(86);
  assert.equal(verdict.kind, "fail");
  assert.match(
    (verdict as { cause: string }).cause,
    /temperature 86 is outside \[-40\.\.85\]/,
  );
});

Deno.test("a value below the range fails", () => {
  const verdict = new TemperatureCheck().accept(-41);
  assert.equal(verdict.kind, "fail");
  assert.match(
    (verdict as { cause: string }).cause,
    /temperature -41 is outside \[-40\.\.85\]/,
  );
  const later = new TemperatureCheck();
  assert.equal(later.accept(22).kind, "continue");
  assert.equal(later.accept(-41).kind, "fail");
});

Deno.test("a step across 85 counts as a wrap", () => {
  const check = new TemperatureCheck();
  check.accept(80);
  assert.deepEqual(check.accept(-30), { kind: "continue" });
  assert.equal(check.progress.wraps, 1);
});

Deno.test("a step of 64 fails", () => {
  const long = new TemperatureCheck();
  long.accept(-40);
  assert.equal(long.accept(24).kind, "fail");
  const short = new TemperatureCheck();
  short.accept(-40);
  assert.equal(short.accept(23).kind, "continue");
});

Deno.test("a repeat is ignored", () => {
  const check = new TemperatureCheck();
  for (const value of [0, 0, 1]) {
    assert.deepEqual(check.accept(value), { kind: "continue" });
  }
  assert.equal(check.progress.received, 3);
});

Deno.test("85 as the first sample does not count", () => {
  const check = new TemperatureCheck();
  assert.deepEqual(check.accept(85), { kind: "continue" });
  assert.deepEqual(check.accept(-40), { kind: "continue" });
});

Deno.test("a fail is sticky", () => {
  const check = new TemperatureCheck();
  const first = check.accept(86);
  assert.equal(first.kind, "fail");
  assert.deepEqual(check.accept(0), first);
  assert.equal(check.progress.received, 1);
});

Deno.test("isPass accepts only a passed progress", () => {
  assert.equal(
    isPass({ received: 1, wraps: 0, sawHighest: false, sawLowestAfter: false }),
    false,
  );
  assert.equal(
    isPass({ received: 5, wraps: 0, sawHighest: true, sawLowestAfter: false }),
    false,
  );
  assert.equal(
    isPass({ received: 0, wraps: 1, sawHighest: true, sawLowestAfter: true }),
    false,
  );
  assert.equal(
    isPass({ received: 5, wraps: 1, sawHighest: false, sawLowestAfter: true }),
    false,
  );
  assert.equal(isPass(undefined), false);
  assert.equal(isPass(3), false);
  const check = new TemperatureCheck();
  for (let value = -40; value <= 85; value++) check.accept(value);
  check.accept(-40);
  assert.equal(isPass(check.progress), true);
});

Deno.test("the generated table addresses Cabin.temperature at 1:1", () => {
  assert.equal(Cabin.iface, 1);
  assert.equal(Cabin.temperature.iface, 1);
  assert.equal(Cabin.temperature.ord, 1);
  assert.equal(Cabin.temperature.kind, "signal");
  assert.equal(Cabin.temperature.payload.type, "veh.cabin.Temperature");
  assert.equal(Cabin.average.reply.type, "veh.cabin.Average");
  assert.equal(Horn.iface, 2);
});

/** The wasm module, read once for the file. */
const bytes = await Deno.readFile(
  new URL("./generated/veh_cabin.wasm", import.meta.url),
);

/** The wasm codec, loaded with the generated table's hash, types and rules. */
const codec: Codec = await loadCodec(bytes, {
  catalogHash: CATALOG_HASH,
  payloadTypes: PAYLOAD_TYPES,
  ruleText: RULES,
});

Deno.test("the codec is loaded with the table's hash and resolves every payload type", () => {
  const id = codec.typeId("veh.cabin.Temperature");
  assert.ok(
    Number.isInteger(id) && id >= 0 && id < PAYLOAD_TYPES.length,
    `type id ${id}`,
  );
  assert.throws(() => codec.typeId("veh.cabin.Nothing"), UnknownPayloadType);
});

Deno.test("encodeTemperature round-trips through decodeTemperature", () => {
  for (const value of [-41, 20, 86]) {
    assert.equal(decodeTemperature(encode(value)), value);
  }
});

Deno.test("RANGE_RULE is 2 plus the index of range in RULES", () => {
  assert.equal(RANGE_RULE, 4);
});

/** The type id of the payload of `Cabin.temperature` in the cabin codec. */
const temperatureId = codec.typeId(Cabin.temperature.payload.type);

Deno.test("the codec accepts 20 and rejects -41 and 86 with the range rule", () => {
  assert.deepEqual(codec.verify(temperatureId, encode(20)), { ok: true });
  for (const value of [-41, 86]) {
    const result = codec.verify(temperatureId, encode(value));
    assert.equal(result.ok, false);
    assert.equal((result as { kind: string }).kind, "rule");
    assert.equal((result as { code: number }).code, 4);
  }
});

Deno.test("signalChecks refuses a value outside the range and passes one inside it", () => {
  const checks = signalChecks(codec);
  const temperature = checks.find((entry) =>
    entry.iface === Cabin.temperature.iface &&
    entry.ord === Cabin.temperature.ord
  );
  assert.ok(temperature !== undefined);
  assert.equal(temperature.check(encode(20)), undefined);
  assert.deepEqual(temperature.check(encode(86)), { rule: "range" });
  assert.deepEqual(temperature.check(encode(-41)), { rule: "range" });
  assert.equal(temperature.check(encode(20).slice(0, 4)), "corrupt");
  assert.deepEqual(
    checks.map(({ iface, ord }) => [iface, ord]),
    [[Cabin.iface, Cabin.temperature.ord], [Horn.iface, Horn.active.ord]],
  );
});

Deno.test("the codec rejects a truncated payload as malformed", () => {
  const result = codec.verify(temperatureId, encode(20).slice(0, 4));
  assert.equal(result.ok, false);
  assert.equal((result as { kind: string }).kind, "malformed");
});

Deno.test("selfTest passes with the cabin codec", () => {
  assert.equal(selfTest(codec), undefined);
});

/** The id the fakes give `veh.cabin.Temperature`: not the real one. */
const FAKE_ID = 7;

Deno.test("the id the fakes give is not the id of Temperature", () => {
  assert.notEqual(FAKE_ID, codec.typeId("veh.cabin.Temperature"));
});

/**
 * A codec whose `verify` gives, for each temperature, the result `judge`
 * returns. It also checks the type name and the type id that the caller
 * passes.
 */
function fakeCodec(judge: (value: number) => VerifyResult): Codec {
  // The id is not Temperature's in the wasm module (see the assertion
  // below), so a caller that passes a constant, or looks another name up,
  // fails here.
  let resolved = false;
  return {
    catalogHash: new Uint8Array(32),
    typeId(name) {
      assert.equal(name, "veh.cabin.Temperature");
      resolved = true;
      return FAKE_ID;
    },
    verify(typeId, bytes) {
      assert.ok(resolved, "verify is called before typeId");
      assert.equal(typeId, FAKE_ID);
      return judge(decodeTemperature(bytes));
    },
  };
}

const RANGE: VerifyResult = { ok: false, kind: "rule", code: RANGE_RULE };

Deno.test("selfTest passes with a codec that checks the range", () => {
  const good = fakeCodec((v) => (v < -40 || v > 85 ? RANGE : { ok: true }));
  assert.equal(selfTest(good), undefined);
});

Deno.test("selfTest fails a codec that accepts every payload", () => {
  assert.match(selfTest(fakeCodec(() => ({ ok: true }))) ?? "", /accepted -41/);
});

Deno.test("selfTest fails a codec that accepts 86", () => {
  const cause = selfTest(fakeCodec((v) => (v < -40 ? RANGE : { ok: true })));
  assert.match(cause ?? "", /accepted 86/);
});

Deno.test("selfTest fails a codec that rejects 20", () => {
  const cause = selfTest(fakeCodec(() => RANGE));
  assert.match(cause ?? "", /rejected 20\b/);
});

Deno.test("selfTest fails a codec that rejects with another kind or code", () => {
  for (
    const other of [
      { ok: false, kind: "malformed", code: 1 },
      { ok: false, kind: "malformed", code: RANGE_RULE },
      { ok: false, kind: "rule", code: 3 },
    ] as VerifyResult[]
  ) {
    const cause = selfTest(
      fakeCodec((v) => (v < -40 || v > 85 ? other : { ok: true })),
    );
    assert.match(cause ?? "", /-41/);
  }
});

Deno.test("verifyPayload gives the cause of each rejection", () => {
  assert.equal(verifyPayload(codec, encode(20)), undefined);
  assert.equal(
    verifyPayload(codec, encode(-41)),
    "the codec rejected the payload with rule code 4 (range)",
  );
  assert.equal(
    verifyPayload(codec, encode(20).slice(0, 4)),
    "the codec found the payload malformed (code 1)",
  );
  // A codec that reports the other kinds, to check the remaining strings,
  // and that gives the id its `verify` must receive.
  const fake = (result: VerifyResult) =>
    ({
      catalogHash: new Uint8Array(32),
      typeId: () => FAKE_ID,
      verify: (typeId: number) => {
        assert.equal(typeId, FAKE_ID);
        return result;
      },
    }) as Codec;
  assert.equal(
    verifyPayload(fake({ ok: false, kind: "rule", code: 3 }), new Uint8Array()),
    "the codec rejected the payload with rule code 3",
  );
  assert.equal(
    verifyPayload(
      fake({ ok: false, kind: "unknown-type", code: -1 }),
      new Uint8Array(),
    ),
    `the codec does not know type id ${FAKE_ID}`,
  );
});

Deno.test("verifyPayload passes the type id of Cabin.temperature", () => {
  assert.equal(
    verifyPayload(fakeCodec(() => ({ ok: true })), encode(20)),
    undefined,
  );
});

/** What a check decided about one value: accepted, or the rule that refused it. */
type Outcome = "ok" | "range" | "step" | "length" | "pattern";

/**
 * One constrained type of cabin.ridl, with its constructor, the generated
 * `encode<Name>` and the values to compare.
 */
interface Subject<T> {
  readonly name: string;
  readonly make: (value: T) => unknown;
  readonly encode: (value: T) => Uint8Array;
  readonly values: readonly T[];
}

function subject<T>(subject: Subject<T>): Subject<T> {
  return subject;
}

/**
 * The outcome of `make<Name>`: "ok", or the rule of the `ConstraintError` it
 * throws. Any other error propagates and fails the test.
 */
function constructed<T>(subject: Subject<T>, value: T): Outcome {
  try {
    subject.make(value);
    return "ok";
  } catch (error) {
    if (error instanceof ConstraintError) return error.rule;
    throw error;
  }
}

/**
 * The outcome of the wasm codec's `verify` over the encoded value: "ok", or
 * the rule that the code names. Another failure fails the test.
 */
function verified<T>(subject: Subject<T>, value: T): Outcome {
  const id = codec.typeId(`veh.cabin.${subject.name}`);
  const result = codec.verify(id, subject.encode(value));
  if (result.ok) return "ok";
  assert.equal(
    result.kind,
    "rule",
    `${subject.name}(${JSON.stringify(value)}): ${JSON.stringify(result)}`,
  );
  const rule = RULES[(result as { code: number }).code - 2];
  assert.ok(
    ["range", "step", "length", "pattern"].includes(rule),
    `${subject.name}: the codec refused with the rule ${rule}`,
  );
  return rule as Outcome;
}

// A value in a list below is one the wire can carry: an integer is inside the
// width of its backing (`Temperature` is an `i8`, `Level` a `u8`, `Average` a
// `u16`, `Window` a `u32`), so the encoded bytes hold the value itself, and
// `verify` judges the same number that `make<Name>` judges. A float list has
// no 0 for `Setpoint`: the `flatc` writer omits a field equal to the schema
// default 0.0, and the wasm codec reports that payload as malformed, which is
// not a constraint verdict (`Tenth` takes 0 and answers as expected).
const subjects = [
  subject<number>({
    name: "Temperature",
    make: makeTemperature,
    encode: (v) => encodeTemperature(v as Temperature),
    values: [-128, -41, -40, -39, 0, 84, 85, 86, 127],
  }),
  subject<number>({
    name: "Level",
    make: makeLevel,
    encode: (v) => encodeLevel(v as Level),
    values: [0, 1, 99, 100, 101, 255],
  }),
  subject<number>({
    name: "Average",
    make: makeAverage,
    encode: (v) => encodeAverage(v as Average),
    values: [0, 1000, 1001, 65535],
  }),
  subject<number>({
    name: "Window",
    make: makeWindow,
    encode: (v) => encodeWindow(v as Window),
    values: [0, 100000, 100001, 4294967295],
  }),
  subject<number>({
    name: "Setpoint",
    make: makeSetpoint,
    encode: (v) => encodeSetpoint(v as Setpoint),
    values: [
      10,
      10.5,
      20,
      20.25,
      20.5,
      29.5,
      29.75,
      30,
      30.5,
      9.5,
      9.75,
      -10.5,
    ],
  }),
  subject<number>({
    name: "Tenth",
    make: makeTenth,
    encode: (v) => encodeTenth(v as Tenth),
    values: [0, 0.1, 0.2, 0.3, 0.5, 0.7, 0.75, 0.05, 0.9, 1, 1.1, -0.1, 2],
  }),
  subject<string>({
    name: "Tag",
    make: makeTag,
    encode: (v) => encodeTag(v as Tag),
    values: [
      "",
      "a",
      "ab",
      "abcd",
      "abcde",
      // An astral character is one character, as `chars()` counts it.
      "\u{1F600}",
      "\u{1F600}\u{1F600}",
      "\u{1F600}\u{1F600}\u{1F600}\u{1F600}",
      "\u{1F600}\u{1F600}\u{1F600}\u{1F600}\u{1F600}",
      "a\u{1F600}",
      "\u00e9\u00e9",
    ],
  }),
  subject<string>({
    name: "Code",
    make: makeCode,
    encode: (v) => encodeCode(v as Code),
    values: [
      "A",
      "ABC",
      "ABCDEFGH",
      "ABCDEFGHI",
      "",
      "abc",
      "A1",
      "AbC",
      "ABC\n",
      "\nABC",
      "\u00c9",
      "\u{1F600}",
    ],
  }),
  subject<Uint8Array>({
    name: "Block",
    make: makeBlock,
    encode: (v) => encodeBlock(v as Block),
    values: [
      Uint8Array.of(),
      Uint8Array.of(1),
      Uint8Array.of(1, 2),
      Uint8Array.of(1, 2, 3, 4),
      Uint8Array.of(1, 2, 3, 4, 5),
    ],
  }),
  // The only constraint of the two is the pattern, so the values keep the
  // length inside its bounds.
  subject<string>({
    name: "Digited",
    make: makeDigited,
    encode: (v) => encodeDigited(v as Digited),
    values: ["1", "a1", "1a", "abc", "", "9", "a b 7"],
  }),
  subject<string>({
    name: "Glyph",
    make: makeGlyph,
    encode: (v) => encodeGlyph(v as Glyph),
    values: ["a", "", "ab", "\u00e9", "\n"],
  }),
] as const;

Deno.test("make<Name> refuses exactly the values that the wasm codec refuses, with the same rule", () => {
  for (const one of subjects) {
    const subject = one as Subject<unknown>;
    for (const value of subject.values) {
      assert.equal(
        constructed(subject, value),
        verified(subject, value),
        `${subject.name}(${JSON.stringify(value)})`,
      );
    }
  }
});

Deno.test("the differential values reach both outcomes of every rule they cover", () => {
  const seen = new Map<string, Set<Outcome>>();
  for (const one of subjects) {
    const subject = one as Subject<unknown>;
    for (const value of subject.values) {
      const outcomes = seen.get(subject.name) ?? new Set<Outcome>();
      outcomes.add(verified(subject, value));
      seen.set(subject.name, outcomes);
    }
  }
  const covered = (name: string, ...expected: Outcome[]) =>
    assert.deepEqual(
      [...(seen.get(name) ?? [])].sort(),
      [...expected].sort(),
      name,
    );
  covered("Temperature", "ok", "range");
  covered("Level", "ok", "range");
  covered("Average", "ok", "range");
  covered("Window", "ok", "range");
  covered("Setpoint", "ok", "range", "step");
  covered("Tenth", "ok", "range", "step");
  covered("Tag", "ok", "length");
  covered("Code", "ok", "length", "pattern");
  covered("Block", "ok", "length");
  covered("Digited", "ok", "pattern");
  covered("Glyph", "ok", "pattern");
});

// Known divergences of the pattern rule (driftsys/ridl#597). `make<Name>` runs
// a JavaScript `RegExp` without the `u` flag, and the wasm codec runs the Rust
// `regex` crate, whose `.` and `\d` follow Unicode. Each case below pins the
// current outcome of both sides. A case that starts to agree fails this test,
// which is the cue to move it into the agreeing values above.
const glyph = subjects[10] as Subject<string>;
const digited = subjects[9] as Subject<string>;

const divergences: readonly {
  readonly subject: Subject<string>;
  readonly value: string;
  readonly why: string;
}[] = [
  {
    // `.` matches one UTF-16 unit in JavaScript, so an astral character is
    // two matches; `regex` matches one scalar value.
    subject: glyph,
    value: "\u{1F600}",
    why: "an astral character is two UTF-16 units to `.`",
  },
  {
    subject: glyph,
    value: "\u{10FFFF}",
    why: "the last scalar value is two UTF-16 units to `.`",
  },
  {
    // JavaScript `.` excludes every line terminator; `regex` excludes `\n`.
    subject: glyph,
    value: "\r",
    why: "`.` excludes a carriage return in JavaScript only",
  },
  {
    subject: glyph,
    value: " ",
    why: "`.` excludes the line separator in JavaScript only",
  },
  {
    // `\d` is `[0-9]` in JavaScript; `regex` matches any Unicode decimal digit.
    subject: digited,
    value: "٣",
    why: "`\\d` matches an Arabic-Indic digit in `regex` only",
  },
  {
    subject: digited,
    value: "１",
    why: "`\\d` matches a fullwidth digit in `regex` only",
  },
];

Deno.test("the pattern engines disagree on the pinned cases (driftsys/ridl#597)", () => {
  assert.equal(glyph.name, "Glyph");
  assert.equal(digited.name, "Digited");
  for (const { subject, value, why } of divergences) {
    const label = `${subject.name}(${JSON.stringify(value)}): ${why}`;
    assert.equal(constructed(subject, value), "pattern", label);
    assert.equal(verified(subject, value), "ok", label);
  }
});
