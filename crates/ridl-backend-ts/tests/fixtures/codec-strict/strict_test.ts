// The behavior of the codec over strict enums and strict enum sets: a
// declared value round-trips, and a value or a bit that the declaration does
// not name is refused with the `variant` rule, in every position a type can
// stand. The recipe `_codec-ts-check` copies this file beside the generated
// files.

import assert from "node:assert/strict";
import * as flatbuffers from "flatbuffers";
import * as codec from "./strict.demo.codec.ts";
import * as fb from "./strict/demo.ts";
import * as t from "./strict.demo.ts";

const variantError = (error: unknown) =>
  error instanceof codec.RuleError && error.rule === "variant";

const pack = (object: { pack(builder: flatbuffers.Builder): flatbuffers.Offset }) => {
  const builder = new flatbuffers.Builder();
  builder.finish(object.pack(builder));
  return builder.asUint8Array();
};

/** A report the buffer holds, with the fields of the test replaced. */
const reportBytes = (overrides: Partial<fb.ReportT> = {}) => {
  const object = new fb.ReportT();
  object.status = 1n;
  object.mode = 2n;
  object.flags = 1;
  object.wide = 1n;
  Object.assign(object, overrides);
  return pack(object);
};

const report = (overrides: Partial<t.Report> = {}): t.Report => ({
  status: t.DtcStatus.ACTIVE,
  mode: t.Mode.HIGH,
  flags: 1 as t.Flags,
  wide: 1n as t.WideFlags,
  list: [],
  sets: [],
  ...overrides,
});

Deno.test("a declared value decodes to the member and encodes back", () => {
  const bytes = codec.encodeDtcStatus(t.DtcStatus.ACTIVE);
  assert.equal(codec.decodeDtcStatus(bytes), t.DtcStatus.ACTIVE);
  assert.equal(codec.decodeDtcStatus(codec.encodeDtcStatus(t.DtcStatus.INACTIVE)), 0);
});

Deno.test("an undeclared enum value is refused on decode with the variant rule", () => {
  for (const raw of [2n, -1n, 7n, 2n ** 53n, 2n ** 63n - 1n, -(2n ** 63n)]) {
    assert.throws(() => codec.decodeDtcStatus(pack(new fb.DtcStatusBoxT(raw))), variantError);
  }
});

Deno.test("an enum value that rounds to a declared number is still refused", () => {
  // 2^53 + 1 is not a safe integer; as a double it is 2^53, never declared.
  assert.throws(
    () => codec.decodeDtcStatus(pack(new fb.DtcStatusBoxT(2n ** 53n + 1n))),
    variantError,
  );
});

Deno.test("a struct field of a strict enum refuses an undeclared value", () => {
  assert.deepEqual(codec.decodeReport(reportBytes()), report());
  assert.throws(
    () => codec.decodeReport(reportBytes({ status: 5n })),
    variantError,
  );
  assert.throws(
    () => codec.decodeReport(reportBytes({ mode: 0n })),
    variantError,
  );
});

Deno.test("an element of a collection of a strict enum refuses an undeclared value", () => {
  const ok = reportBytes({ list: [0n, 1n] });
  assert.deepEqual(codec.decodeReport(ok).list, [0, 1]);
  assert.throws(
    () => codec.decodeReport(reportBytes({ list: [1n, 9n] })),
    variantError,
  );
});

Deno.test("an optional strict enum refuses an undeclared value and reads absent as undefined", () => {
  assert.equal(codec.decodeReport(reportBytes()).maybe, undefined);
  assert.equal(codec.decodeReport(reportBytes({ maybe: 1n })).maybe, 1);
  assert.throws(
    () => codec.decodeReport(reportBytes({ maybe: 4n })),
    variantError,
  );
});

Deno.test("a tuple position of a strict enum refuses an undeclared value", () => {
  const declared: codec.ControlSelectParams = { status: t.DtcStatus.ACTIVE, mode: t.Mode.HIGH };
  assert.deepEqual(
    codec.decodeControlSelectParams(codec.encodeControlSelectParams(declared)),
    declared,
  );
  const tuple = new fb.ControlSelectParamsT(1n, 3n);
  assert.throws(() => codec.decodeControlSelectParams(pack(tuple)), variantError);
  assert.throws(
    () => codec.decodeControlSelectParams(pack(new fb.ControlSelectParamsT(8n, 1n))),
    variantError,
  );
});

Deno.test("a union arm of a strict enum or a strict enum set refuses an undeclared value", () => {
  const status: t.Pick = { kind: "status", value: t.DtcStatus.ACTIVE };
  assert.deepEqual(codec.decodePick(codec.encodePick(status)), status);
  const flags: t.Pick = { kind: "flags", value: 0b1011 as t.Flags };
  assert.deepEqual(codec.decodePick(codec.encodePick(flags)), flags);
  assert.throws(
    () =>
      codec.decodePick(
        pack(new fb.PickT(fb.PickUnion.status, new fb.PickStatusBoxT(6n))),
      ),
    variantError,
  );
  assert.throws(
    () =>
      codec.decodePick(
        pack(new fb.PickT(fb.PickUnion.flags, new fb.PickFlagsBoxT(0b100))),
      ),
    variantError,
  );
});

Deno.test("a strict enum set refuses a bit it does not declare", () => {
  for (const declared of [0, 0b1, 0b10, 0b1000, 0b1011]) {
    const value = declared as t.Flags;
    assert.equal(codec.decodeFlags(codec.encodeFlags(value)), value);
  }
  for (const undeclared of [0b100, 0b10000, 0b1111, 0x80]) {
    assert.throws(
      () => codec.decodeFlags(pack(new fb.FlagsBoxT(undeclared))),
      variantError,
    );
  }
});

Deno.test("a wide strict enum set refuses a bit it does not declare", () => {
  const declared = ((1n << 40n) | 1n) as t.WideFlags;
  assert.equal(codec.decodeWideFlags(codec.encodeWideFlags(declared)), declared);
  for (const undeclared of [1n << 41n, 1n << 63n, (1n << 40n) | 2n, -1n]) {
    assert.throws(
      () => codec.decodeWideFlags(pack(new fb.WideFlagsBoxT(undeclared))),
      variantError,
    );
  }
});

Deno.test("a struct field and a collection of strict enum sets refuse an undeclared bit", () => {
  const ok = reportBytes({ sets: [0b1, 0b1011], maybeFlags: 0b10 });
  assert.deepEqual(codec.decodeReport(ok).sets, [1, 11]);
  assert.equal(codec.decodeReport(ok).maybeFlags, 2);
  assert.throws(() => codec.decodeReport(reportBytes({ flags: 0b100 })), variantError);
  assert.throws(() => codec.decodeReport(reportBytes({ wide: 1n << 41n })), variantError);
  assert.throws(() => codec.decodeReport(reportBytes({ sets: [1, 0b100] })), variantError);
  assert.throws(
    () => codec.decodeReport(reportBytes({ maybeFlags: 0b10000 })),
    variantError,
  );
});

Deno.test("an absent strict enum with no zero member is still reported as absent", () => {
  const bytes = pack(new fb.ReportT());
  assert.throws(
    () => codec.decodeReport(bytes),
    (error: unknown) => error instanceof RangeError && !(error instanceof codec.RuleError),
  );
});
