import assert from "node:assert/strict";
import * as codec from "./names.codec.ts";
import { makeConstraintError, makecodePointLength } from "./names.ts";

Deno.test("an interface may take a name that the runtime package exports", () => {
  assert.equal(codec.Codec.level.kind, "signal");
  assert.equal(codec.CodecError.changed.kind, "event");
  assert.equal(codec.SignalCheckEntry.report.kind, "command");
  assert.equal(codec.PayloadCodec.read.kind, "query");
  assert.equal(codec.checkWellFormed.level.kind, "signal");
});

Deno.test("a payload with names of the runtime package round-trips", () => {
  const value = {
    label: makecodePointLength("speed"),
    value: makeConstraintError(42),
  };
  assert.deepEqual(codec.decodeReading(codec.encodeReading(value)), value);
});
