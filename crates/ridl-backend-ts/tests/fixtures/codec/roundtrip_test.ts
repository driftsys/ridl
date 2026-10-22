// The round trip `decode<Name>(encode<Name>(v))` deep-equals `v` over the
// kinds cabin does not exercise: a union, a tuple, an array, a map, an
// optional field, an induced tuple and an enum with no zero member. The
// recipe `_codec-ts-check` copies this file beside the generated files.

import assert from "node:assert/strict";
import * as flatbuffers from "flatbuffers";
import * as codec from "./veh.common.codec.ts";
import * as fb from "./veh/common.ts";
import type * as ridl_std from "./ridl.std.ts";
import * as t from "./veh.common.ts";

const speed = (value: number): t.Speed => value as t.Speed;
const label = (value: string): ridl_std.Label => value as ridl_std.Label;
const name = (value: string): ridl_std.Name => value as ridl_std.Name;

Deno.test("a union round-trips through each arm", () => {
  const ok: t.SensorResult = {
    kind: "ok",
    value: { value: speed(12.5), timestamp: 7n as ridl_std.Timestamp },
  };
  assert.deepEqual(codec.decodeSensorResult(codec.encodeSensorResult(ok)), ok);
  const err: t.SensorResult = {
    kind: "err",
    value: { code: 3 as t.Counter, message: "fault" as ridl_std.Message },
  };
  assert.deepEqual(codec.decodeSensorResult(codec.encodeSensorResult(err)), err);
});

Deno.test("a union arm keeps its declared name as the kind", () => {
  const outcome: t.Outcome = {
    kind: "errFault",
    value: { code: 9 as t.Counter, message: "late" as ridl_std.Message },
  };
  const decoded = codec.decodeOutcome(codec.encodeOutcome(outcome));
  assert.deepEqual(decoded, outcome);
  assert.equal(decoded.kind, "errFault");
});

Deno.test("a tuple, an array and a map round-trip", () => {
  const bounds: t.SensorBounds = {
    range: { min: speed(1), max: speed(2) },
    readings: [1, 2, 3, 4, 5, 6, 7, 8].map(speed),
    labels: [label("a")],
    meta: [[label("k"), name("v")]],
  };
  assert.deepEqual(codec.decodeSensorBounds(codec.encodeSensorBounds(bounds)), bounds);
});

Deno.test("an optional field round-trips present and absent", () => {
  const present: t.DriverProfile = {
    name: name("ann"),
    speed: speed(30),
    override: speed(40),
    gears: 6 as t.DriverProfile["gears"],
  };
  assert.deepEqual(codec.decodeDriverProfile(codec.encodeDriverProfile(present)), present);
  const absent: t.DriverProfile = {
    name: name("bob"),
    speed: speed(30),
    gears: 5 as t.DriverProfile["gears"],
  };
  const decoded = codec.decodeDriverProfile(codec.encodeDriverProfile(absent));
  assert.deepEqual(decoded, absent);
  assert.equal("override" in decoded, false);
});

Deno.test("a two-parameter call round-trips by name", () => {
  const params: codec.SensorsSetLimitParams = { limit: speed(10), actual: speed(20) };
  assert.deepEqual(
    codec.decodeSensorsSetLimitParams(codec.encodeSensorsSetLimitParams(params)),
    params,
  );
  assert.equal(codec.Sensors.setLimit.request.type, "veh.common.SensorsSetLimitParams");
  assert.equal(codec.Sensors.setLimit.ord, 5);
  assert.equal(codec.Sensors.result.ord, 3);
});

Deno.test("a tuple reply round-trips by name", () => {
  const reply: codec.SensorsSpanReply = { min: speed(1), max: speed(2) };
  assert.deepEqual(codec.decodeSensorsSpanReply(codec.encodeSensorsSpanReply(reply)), reply);
  assert.equal(codec.Sensors.span.reply.type, "veh.common.SensorsSpanReply");
});

Deno.test("an enum set, an enum and an enum field round-trip", () => {
  const flags = 3 as t.WarningFlags;
  assert.equal(codec.decodeWarningFlags(codec.encodeWarningFlags(flags)), flags);
  assert.equal(
    codec.decodeGearPosition(codec.encodeGearPosition(t.GearPosition.REVERSE)),
    t.GearPosition.REVERSE,
  );
  const setting: t.Setting = { mode: t.Mode.HIGH };
  assert.deepEqual(codec.decodeSetting(codec.encodeSetting(setting)), setting);
});

Deno.test("an absent required enum field throws on decode", () => {
  const builder = new flatbuffers.Builder();
  builder.finish(new fb.SettingT().pack(builder));
  assert.throws(
    () => codec.decodeSetting(builder.asUint8Array()),
    (error: unknown) =>
      error instanceof RangeError &&
      error.message === "field mode of veh.common.Setting is absent",
  );
});

Deno.test("a foreign scalar payload goes through its own package's codec", () => {
  const value = name("ann");
  assert.equal(codec.Sensors.profile.request.type, "ridl.std.Name");
  assert.equal(
    codec.Sensors.profile.request.decode(codec.Sensors.profile.request.encode(value)),
    value,
  );
});
