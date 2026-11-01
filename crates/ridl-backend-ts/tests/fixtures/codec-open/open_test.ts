// The behavior of the codec over open enums and open enum sets: an
// undeclared value is kept, a value outside the safe integers or outside the
// width of a set is refused with the rule that names it. The recipe
// `_codec-ts-check` copies this file beside the generated files.

import assert from "node:assert/strict";
import * as flatbuffers from "flatbuffers";
import * as codec from "./open.demo.codec.ts";
import * as fb from "./open/demo.ts";
import * as t from "./open.demo.ts";

const ruleError = (rule: string) => (error: unknown) =>
  error instanceof codec.RuleError && error.rule === rule;

const report = (overrides: Partial<t.Report> = {}): t.Report => ({
  status: t.DtcStatus.ACTIVE,
  mode: t.Mode.HIGH,
  flags: 1 as t.Flags,
  wide: 1n as t.WideFlags,
  list: [],
  ...overrides,
});

Deno.test("a declared value decodes to the member and encodes back", () => {
  const bytes = codec.encodeDtcStatus(t.DtcStatus.ACTIVE);
  assert.equal(codec.decodeDtcStatus(bytes), t.DtcStatus.ACTIVE);
});

Deno.test("an undeclared value decodes to unknown and encodes to the same bytes", () => {
  const bytes = codec.encodeDtcStatus({ unknown: 7 });
  assert.deepEqual(codec.decodeDtcStatus(bytes), { unknown: 7 });
  assert.deepEqual(codec.encodeDtcStatus(codec.decodeDtcStatus(bytes)), bytes);
  // The raw integer 7 is what the buffer holds.
  const box = fb.DtcStatusBox.getRootAsDtcStatusBox(new flatbuffers.ByteBuffer(bytes));
  assert.equal(box.value(), 7n);
});

Deno.test("a negative undeclared value is kept", () => {
  const value = { unknown: -3 };
  assert.deepEqual(codec.decodeDtcStatus(codec.encodeDtcStatus(value)), value);
});

Deno.test("a value beyond the safe integers is refused on decode with the variant rule", () => {
  const builder = new flatbuffers.Builder();
  builder.finish(new fb.DtcStatusBoxT(2n ** 53n).pack(builder));
  assert.throws(() => codec.decodeDtcStatus(builder.asUint8Array()), ruleError("variant"));
  const limit = new flatbuffers.Builder();
  limit.finish(new fb.DtcStatusBoxT(2n ** 53n - 1n).pack(limit));
  assert.deepEqual(codec.decodeDtcStatus(limit.asUint8Array()), { unknown: 2 ** 53 - 1 });
});

Deno.test("a value beyond the safe integers is refused on encode with the variant rule", () => {
  assert.throws(() => codec.encodeDtcStatus({ unknown: 2 ** 53 }), ruleError("variant"));
  assert.throws(() => codec.encodeDtcStatus({ unknown: 1.5 }), ruleError("variant"));
});

Deno.test("a struct with open fields round-trips, an unknown value included", () => {
  const value = report({
    status: { unknown: 9 },
    mode: { unknown: 0 },
    flags: 0b1000_0001 as t.Flags,
    wide: (1n << 40n | 1n << 41n) as t.WideFlags,
    list: [t.DtcStatus.INACTIVE, { unknown: 5 }],
    maybe: { unknown: 6 },
  });
  assert.deepEqual(codec.decodeReport(codec.encodeReport(value)), value);
});

Deno.test("an absent open enum without a zero member decodes to unknown 0", () => {
  const builder = new flatbuffers.Builder();
  builder.finish(new fb.ReportT().pack(builder));
  const decoded = codec.decodeReport(builder.asUint8Array());
  assert.deepEqual(decoded.mode, { unknown: 0 });
});

Deno.test("an open enum set keeps an undeclared bit and reports it", () => {
  const value = 0b1000_0001 as t.Flags;
  assert.equal(codec.decodeFlags(codec.encodeFlags(value)), value);
  assert.equal(t.FlagsBits.unknownBits(value), 0b1000_0000);
  assert.equal(t.FlagsBits.unknownBits(3 as t.Flags), 0);
});

Deno.test("the unknown bits of a wide open enum set are a bigint", () => {
  const value = ((1n << 40n) | (1n << 50n)) as t.WideFlags;
  assert.equal(t.WideFlagsBits.unknownBits(value), 1n << 50n);
  assert.equal(codec.decodeWideFlags(codec.encodeWideFlags(value)), value);
});

Deno.test("an open enum set value outside its wire width is refused with the range rule", () => {
  assert.throws(() => codec.encodeFlags(256 as t.Flags), ruleError("range"));
  assert.throws(() => codec.encodeFlags(-1 as t.Flags), ruleError("range"));
  assert.throws(() => codec.encodeFlags(1.5 as t.Flags), ruleError("range"));
  assert.throws(() => codec.encodeWideFlags(-1n as t.WideFlags), ruleError("range"));
  assert.throws(() => codec.encodeWideFlags((1n << 64n) as t.WideFlags), ruleError("range"));
  assert.throws(() => codec.encodeReport(report({ flags: 256 as t.Flags })), ruleError("range"));
});

Deno.test("an empty open enum decodes every value to unknown", () => {
  for (const raw of [0, 3, -1]) {
    const value = { value: { unknown: raw } };
    assert.deepEqual(codec.decodeEmptyReport(codec.encodeEmptyReport(value)), value);
  }
  const builder = new flatbuffers.Builder();
  builder.finish(new fb.EmptyReportT().pack(builder));
  assert.deepEqual(codec.decodeEmptyReport(builder.asUint8Array()), { value: { unknown: 0 } });
});

Deno.test("a union arm of an open enum or an open enum set round-trips", () => {
  const status: t.Pick = { kind: "status", value: { unknown: 4 } };
  assert.deepEqual(codec.decodePick(codec.encodePick(status)), status);
  const flags: t.Pick = { kind: "flags", value: 0b1000_0001 as t.Flags };
  assert.deepEqual(codec.decodePick(codec.encodePick(flags)), flags);
});
