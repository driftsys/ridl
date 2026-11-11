// The signal and event methods of the `SensorsClient` that the codec module
// writes, over the in-process fake port of the runtime package and a scripted
// codec. The recipe `_codec-ts-check` copies this file beside the generated
// files.

import assert from "node:assert/strict";
import {
  type Codec,
  type RawOccurrence,
  type RawSample,
  type VerifyResult,
} from "@driftsys/ridl-runtime";
import { FakePort } from "@ridl-runtime-test/tests/fake_port.ts";
import * as codec from "./veh.common.codec.ts";
import * as t from "./veh.common.ts";
import type * as ridl_std from "./ridl.std.ts";

const speed = (value: number): t.Speed => value as t.Speed;
const reading = (value: number): t.SensorReading => ({
  value: speed(value),
  timestamp: 7n as ridl_std.Timestamp,
});
const encoded = (value: number) =>
  codec.Sensors.latest.payload.encode(reading(value));
const envelope = { stamp: 3n, seq: 4n };

/** A codec that numbers each type name on first use and runs `verify`. */
function scripted(verify: (bytes: Uint8Array) => VerifyResult) {
  const ids = new Map<string, number>();
  const loaded: Codec = {
    catalogHash: codec.CATALOG_HASH.slice(),
    typeId(name) {
      if (!ids.has(name)) ids.set(name, ids.size + 1);
      return ids.get(name)!;
    },
    verify: (_, bytes) => verify(bytes),
  };
  return loaded;
}

const accept = (): VerifyResult => ({ ok: true });
const port = () => new FakePort(codec.CATALOG_HASH.slice());
const { iface } = codec.Sensors;
const latest = codec.Sensors.latest.ord;

const live = (payload: Uint8Array): RawSample => ({
  provenance: "live",
  envelope,
  payload,
  lastGood: payload,
});

Deno.test("before the first publish the signal reads as the init sample", () => {
  const client = codec.SensorsClient.bind(port(), scripted(accept));
  assert.deepEqual(client.latest(), {
    provenance: "init",
    envelope: { stamp: 0n, seq: 0n },
    value: t.initSensorReading(),
  });
});

Deno.test("a live sample holds the decoded value at once", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  fake.publish(iface, latest, live(encoded(12.5)));
  assert.deepEqual(client.latest(), {
    provenance: "live",
    envelope,
    value: reading(12.5),
  });
  // The held value stays until the next publish.
  assert.deepEqual(client.latest().value, reading(12.5));
  fake.publish(iface, latest, live(encoded(20)));
  assert.deepEqual(client.latest().value, reading(20));
});

Deno.test("an init sample carries the last good value, else the init value", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  fake.publish(iface, latest, {
    provenance: "init",
    envelope,
    payload: null,
    lastGood: null,
  });
  assert.deepEqual(client.latest().value, t.initSensorReading());
  assert.equal(client.latest().provenance, "init");
  fake.publish(iface, latest, {
    provenance: "init",
    envelope,
    payload: null,
    lastGood: encoded(5),
  });
  assert.deepEqual(client.latest().value, reading(5));
});

Deno.test("an invalid sample keeps the last good value and its detection", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  fake.publish(iface, latest, {
    provenance: "invalid",
    envelope,
    payload: null,
    lastGood: null,
  });
  assert.deepEqual(client.latest(), {
    provenance: "invalid",
    envelope,
    value: t.initSensorReading(),
  });
  fake.publish(iface, latest, {
    provenance: "invalid",
    envelope,
    payload: null,
    lastGood: encoded(6),
    detected: { rule: "range" },
  });
  assert.deepEqual(client.latest(), {
    provenance: "invalid",
    envelope,
    value: reading(6),
    detected: { rule: "range" },
  });
  fake.publish(iface, latest, {
    provenance: "invalid",
    envelope,
    payload: null,
    lastGood: null,
    detected: "corrupt",
  });
  assert.deepEqual(client.latest().detected, "corrupt");
  assert.deepEqual(client.latest().value, t.initSensorReading());
});

Deno.test("a live payload that fails verification is read as invalid with the rule", () => {
  const bad = encoded(99);
  const fake = port();
  const client = codec.SensorsClient.bind(
    fake,
    scripted((bytes) =>
      bytes.length === bad.length && bytes.every((b, i) => b === bad[i])
        ? { ok: false, kind: "rule", code: 4, rule: "range" }
        : { ok: true }
    ),
  );
  fake.publish(iface, latest, {
    provenance: "live",
    envelope,
    payload: bad,
    lastGood: encoded(8),
  });
  assert.deepEqual(client.latest(), {
    provenance: "invalid",
    envelope,
    value: reading(8),
    detected: { rule: "range" },
  });
});

Deno.test("readLatest waits for the next publish and passes its options", async () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  fake.publish(iface, latest, live(encoded(1)));
  const controller = new AbortController();
  const opts = { bound: 50, signal: controller.signal };
  let settled = false;
  const next = client.readLatest(opts).then((sample) => {
    settled = true;
    return sample;
  });
  await Promise.resolve();
  assert.equal(settled, false);
  const read = fake.calls.find((call) => call.method === "read")!;
  assert.equal(read.iface, iface);
  assert.equal(read.ord, latest);
  assert.equal(read.opts, opts);
  fake.publish(iface, latest, live(encoded(2)));
  assert.deepEqual(await next, {
    provenance: "live",
    envelope,
    value: reading(2),
  });
});

Deno.test("watchLatest delivers every publish until it is removed", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  const seen: number[] = [];
  const remove = client.watchLatest((sample) =>
    seen.push(Number(sample.value.value))
  );
  assert.equal(fake.watcherCount(iface, latest), 1);
  fake.publish(iface, latest, live(encoded(1)));
  fake.publish(iface, latest, live(encoded(2)));
  remove();
  fake.publish(iface, latest, live(encoded(3)));
  assert.deepEqual(seen, [1, 2]);
  assert.equal(fake.watcherCount(iface, latest), 0);
});

Deno.test("bind registers the check of every signal with a method", () => {
  const fake = port();
  assert.equal(fake.calls.length, 0);
  const bad = Uint8Array.of(9, 9, 9);
  codec.SensorsClient.bind(
    fake,
    scripted((bytes) =>
      bytes[0] === 9
        ? { ok: false, kind: "rule", code: 4, rule: "range" }
        : { ok: true }
    ),
  );
  const registered = fake.calls
    .filter((call) => call.method === "checkWith")
    .map((call) => [call.iface, call.ord]);
  const expected = [
    codec.Sensors.latest,
    codec.Sensors.setting,
    codec.Sensors.note,
    codec.Sensors.pick,
  ].map((entry) => [entry.iface, entry.ord]);
  assert.deepEqual(registered, expected);
  assert.deepEqual(fake.calls.length, expected.length);
  for (const [i, o] of expected) {
    const check = fake.checks.get(`${i}/${o}`);
    assert.ok(check, `no check for ${i}/${o}`);
    assert.equal(check(Uint8Array.of(1)), undefined);
    assert.deepEqual(check(bad), { rule: "range" });
  }
  // A signal with no method gets no registration.
  assert.equal(fake.checks.has(`${iface}/${codec.Sensors.extras.ord}`), false);
});

Deno.test("a subscribe method passes its options to the port", async () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  const opts = { bound: 25, signal: new AbortController().signal };
  await client.subscribeLatest(opts);
  await client.subscribeResult(opts);
  await client.subscribeBounds();
  const subscribed = fake.calls.filter((call) => call.method === "subscribe");
  assert.deepEqual(
    subscribed.map((call) => [call.iface, call.ord]),
    [
      [iface, latest],
      [iface, codec.Sensors.result.ord],
      [iface, codec.Sensors.bounds.ord],
    ],
  );
  assert.equal(subscribed[0].opts, opts);
  assert.equal(subscribed[1].opts, opts);
  assert.equal(subscribed[2].opts, undefined);
});

const occurrence = (ord: number, payload: Uint8Array): RawOccurrence => ({
  iface,
  ord,
  envelope,
  payload,
});

Deno.test("onEvent routes each occurrence by ordinal as a discriminated union", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  const seen: codec.SensorsEvent[] = [];
  client.onEvent((event) => seen.push(event));
  const result: t.SensorResult = { kind: "ok", value: reading(4) };
  const bounds: t.BoundsSummary = {
    range: { min: speed(1), max: speed(2) },
    readings: Array.from({ length: 8 }, () => speed(3)),
  };
  fake.occur(
    occurrence(
      codec.Sensors.result.ord,
      codec.Sensors.result.payload.encode(result),
    ),
  );
  fake.occur(
    occurrence(
      codec.Sensors.bounds.ord,
      codec.Sensors.bounds.payload.encode(bounds),
    ),
  );
  // An ordinal that is no event of the interface is ignored.
  fake.occur(occurrence(codec.Sensors.latest.ord, Uint8Array.of(1)));
  assert.deepEqual(seen, [
    {
      event: "result",
      occurrence: {
        iface,
        ord: codec.Sensors.result.ord,
        envelope,
        value: result,
      },
    },
    {
      event: "bounds",
      occurrence: {
        iface,
        ord: codec.Sensors.bounds.ord,
        envelope,
        value: bounds,
      },
    },
  ]);
});

Deno.test("an occurrence the port refused is delivered with detected and no value", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  const seen: codec.SensorsEvent[] = [];
  client.onEvent((event) => seen.push(event));
  fake.occur({
    ...occurrence(codec.Sensors.result.ord, new Uint8Array()),
    detected: "corrupt",
  });
  assert.equal(seen.length, 1);
  assert.equal(seen[0].event, "result");
  assert.equal(seen[0].occurrence.detected, "corrupt");
  assert.equal("value" in seen[0].occurrence, false);
  // The refusal decides, whatever bytes come with it.
  fake.occur({
    ...occurrence(
      codec.Sensors.result.ord,
      codec.Sensors.result.payload.encode({ kind: "ok", value: reading(1) }),
    ),
    detected: "corrupt",
  });
  assert.equal(seen[1].occurrence.detected, "corrupt");
  assert.equal(seen[1].occurrence.value, undefined);
});

Deno.test("an occurrence that fails verification is delivered as corrupt", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(
    fake,
    scripted(() => ({ ok: false, kind: "malformed", code: 1 })),
  );
  const seen: codec.SensorsEvent[] = [];
  client.onEvent((event) => seen.push(event));
  fake.occur(occurrence(codec.Sensors.bounds.ord, Uint8Array.of(1, 2)));
  assert.equal(seen.length, 1);
  assert.equal(seen[0].event, "bounds");
  assert.equal(seen[0].occurrence.detected, "corrupt");
  assert.equal(seen[0].occurrence.value, undefined);
});

Deno.test("removing an event listener stops its delivery", () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  let count = 0;
  const remove = client.onEvent(() => count++);
  const bytes = codec.Sensors.result.payload.encode({
    kind: "ok",
    value: reading(1),
  });
  fake.occur(occurrence(codec.Sensors.result.ord, bytes));
  remove();
  fake.occur(occurrence(codec.Sensors.result.ord, bytes));
  assert.equal(count, 1);
});

Deno.test("the event union narrows by its tag and the sample value is never absent", () => {
  const client = codec.SensorsClient.bind(port(), scripted(accept));
  // Never called: only the type checker reads these lines.
  const unused = () => {
    client.onEvent((event) => {
      if (event.event === "result") {
        const value: t.SensorResult | undefined = event.occurrence.value;
        assert.ok(value === undefined || value.kind.length > 0);
      } else {
        const value: t.BoundsSummary | undefined = event.occurrence.value;
        assert.ok(value === undefined || value.readings.length > 0);
      }
      // @ts-expect-error there is no event named `nonsense`
      assert.ok(event.event === "nonsense");
    });
    const sample: t.SensorReading = client.latest().value;
    assert.ok(sample);
    // @ts-expect-error a signal sample is not a promise
    client.latest().then;
  };
  assert.equal(typeof unused, "function");
});

Deno.test("readLatest and watchLatest decode init and invalid samples", async () => {
  const fake = port();
  const client = codec.SensorsClient.bind(fake, scripted(accept));
  const seen: ReturnType<typeof client.latest>[] = [];
  client.watchLatest((sample) => seen.push(sample));
  const init: RawSample = {
    provenance: "init",
    envelope,
    payload: null,
    lastGood: encoded(5),
  };
  const next = client.readLatest();
  fake.publish(iface, latest, init);
  assert.deepEqual(await next, {
    provenance: "init",
    envelope,
    value: reading(5),
  });
  const invalid: RawSample = {
    provenance: "invalid",
    envelope,
    payload: null,
    lastGood: null,
    detected: "corrupt",
  };
  const after = client.readLatest();
  fake.publish(iface, latest, invalid);
  assert.deepEqual(await after, {
    provenance: "invalid",
    envelope,
    value: t.initSensorReading(),
    detected: "corrupt",
  });
  assert.deepEqual(seen.map((sample) => sample.provenance), [
    "init",
    "invalid",
  ]);
  assert.equal(seen[1].detected, "corrupt");
  assert.deepEqual(seen[1].value, t.initSensorReading());
});
