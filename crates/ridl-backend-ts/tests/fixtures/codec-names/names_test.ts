import assert from "node:assert/strict";
import type { Codec } from "@driftsys/ridl-runtime";
import { FakePort } from "@ridl-runtime-test/tests/fake_port.ts";
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

Deno.test("a client class of an interface named like a runtime type calls its members", async () => {
  const port = new FakePort(codec.CATALOG_HASH.slice());
  const loaded: Codec = {
    catalogHash: codec.CATALOG_HASH.slice(),
    typeId: () => 1,
    verify: () => ({ ok: true }),
  };
  const value = {
    label: makecodePointLength("speed"),
    value: makeConstraintError(42),
  };
  port.onQuery = () => Promise.resolve(codec.encodeReading(value));
  const asked = await codec.PayloadCodecClient.bind(port, loaded).read();
  assert.deepEqual(asked, value);
  await codec.SignalCheckEntryClient.bind(port, loaded).report(value);
  assert.deepEqual(port.calls.map((call) => call.method), ["query", "command"]);
});
