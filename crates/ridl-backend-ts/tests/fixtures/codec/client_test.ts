// The `SensorsClient` that the codec module writes for the commands and queries
// of the interface `Sensors`, over the in-process fake port of the runtime
// package and a scripted codec. The recipe `_codec-ts-check` copies this file
// beside the generated files.

import assert from "node:assert/strict";
import {
  CallError,
  CatalogMismatch,
  type Codec,
  CodecError,
  type VerifyResult,
} from "@driftsys/ridl-runtime";
import { FakePort } from "@ridl-runtime-test/tests/fake_port.ts";
import * as codec from "./veh.common.codec.ts";
import type * as ridl_std from "./ridl.std.ts";
import type * as t from "./veh.common.ts";

const speed = (value: number): t.Speed => value as t.Speed;

/** A codec that numbers each type name on first use and records every call. */
function scripted(verify: (id: number, bytes: Uint8Array) => VerifyResult) {
  const resolved: string[] = [];
  const verified: { id: number; bytes: Uint8Array }[] = [];
  const ids = new Map<string, number>();
  const loaded: Codec = {
    catalogHash: codec.CATALOG_HASH.slice(),
    typeId(name) {
      resolved.push(name);
      if (!ids.has(name)) ids.set(name, ids.size + 1);
      return ids.get(name)!;
    },
    verify(id, bytes) {
      verified.push({ id, bytes });
      return verify(id, bytes);
    },
  };
  return { loaded, resolved, verified, ids };
}

const accept = (): VerifyResult => ({ ok: true });
const port = () => new FakePort(codec.CATALOG_HASH.slice());

Deno.test("a command sends the verified request bytes and resolves", async () => {
  const { loaded, verified, ids } = scripted(accept);
  const fake = port();
  const client = codec.SensorsClient.bind(fake, loaded);
  assert.equal(await client.setLimit(speed(10), speed(20)), undefined);
  const bytes = codec.Sensors.setLimit.request.encode({
    limit: speed(10),
    actual: speed(20),
  });
  assert.equal(fake.calls.length, 1);
  assert.equal(fake.calls[0].method, "command");
  assert.equal(fake.calls[0].iface, codec.Sensors.iface);
  assert.equal(fake.calls[0].ord, codec.Sensors.setLimit.ord);
  assert.deepEqual(fake.calls[0].args, bytes);
  assert.deepEqual(verified, [{
    id: ids.get(codec.Sensors.setLimit.request.type),
    bytes,
  }]);
});

Deno.test("a query with one named parameter sends that value as the request", async () => {
  const { loaded } = scripted(accept);
  const fake = port();
  const profile: t.DriverProfile = {
    name: "ann" as ridl_std.Name,
    speed: speed(3),
    gears: 6 as t.DriverProfile["gears"],
  };
  fake.onQuery = () =>
    Promise.resolve(codec.Sensors.profile.reply.encode(profile));
  const client = codec.SensorsClient.bind(fake, loaded);
  const reply = await client.profile("ann" as ridl_std.Name);
  assert.deepEqual(reply, profile);
  assert.deepEqual(
    fake.calls[0].args,
    codec.Sensors.profile.request.encode("ann" as ridl_std.Name),
  );
  assert.equal(fake.calls[0].method, "query");
});

Deno.test("a query verifies the reply, then decodes it", async () => {
  const { loaded, verified, ids } = scripted(accept);
  const fake = port();
  const reply = codec.Sensors.span.reply.encode({
    min: speed(1),
    max: speed(2),
  });
  fake.onQuery = () => Promise.resolve(reply);
  const client = codec.SensorsClient.bind(fake, loaded);
  assert.deepEqual(await client.span(), { min: speed(1), max: speed(2) });
  assert.deepEqual(verified.at(-1), {
    id: ids.get(codec.Sensors.span.reply.type),
    bytes: reply,
  });
});

Deno.test("a reply that fails a rule rejects with the contract error and the rule", async () => {
  const { loaded, ids } = scripted((id) =>
    id === ids.get(codec.Sensors.span.reply.type)
      ? { ok: false, kind: "rule", code: 4, rule: "range" }
      : { ok: true }
  );
  const fake = port();
  fake.onQuery = () =>
    Promise.resolve(
      codec.Sensors.span.reply.encode({ min: speed(1), max: speed(2) }),
    );
  const client = codec.SensorsClient.bind(fake, loaded);
  await assert.rejects(client.span(), (error: unknown) => {
    assert.ok(error instanceof CallError);
    assert.deepEqual(error.failure, {
      category: "contract",
      contract: { category: "invalid-value", rule: "range" },
    });
    return true;
  });
});

Deno.test("a malformed reply rejects as corrupt", async () => {
  const { loaded, ids } = scripted((id) =>
    id === ids.get(codec.Sensors.span.reply.type)
      ? { ok: false, kind: "malformed", code: 1 }
      : { ok: true }
  );
  const fake = port();
  fake.onQuery = () => Promise.resolve(Uint8Array.of(1, 2, 3));
  const client = codec.SensorsClient.bind(fake, loaded);
  await assert.rejects(client.span(), (error: unknown) => {
    assert.ok(error instanceof CallError);
    assert.deepEqual(error.failure, { category: "corrupt" });
    return true;
  });
});

Deno.test("a rule text the module does not know rejects with a CodecError", async () => {
  const { loaded } = scripted(() => ({
    ok: false,
    kind: "rule",
    code: 9,
    rule: "nonsense",
  }));
  const fake = port();
  const client = codec.SensorsClient.bind(fake, loaded);
  await assert.rejects(
    client.setLimit(speed(1), speed(2)),
    CodecError,
  );
  assert.equal(fake.calls.length, 0);
});

Deno.test("a request that fails verification sends nothing", async () => {
  const { loaded } = scripted(() => ({
    ok: false,
    kind: "rule",
    code: 4,
    rule: "range",
  }));
  const fake = port();
  const client = codec.SensorsClient.bind(fake, loaded);
  await assert.rejects(
    client.setLimit(speed(1), speed(2)),
    (error: unknown) => {
      assert.ok(error instanceof CallError);
      assert.deepEqual(error.failure, {
        category: "contract",
        contract: { category: "invalid-value", rule: "range" },
      });
      return true;
    },
  );
  assert.equal(fake.calls.length, 0);
});

Deno.test("the bound defaults to the declared maximum and opts.bound overrides it", async () => {
  const { loaded } = scripted(accept);
  const fake = port();
  fake.onQuery = () =>
    Promise.resolve(
      codec.Sensors.span.reply.encode({ min: speed(1), max: speed(2) }),
    );
  const client = codec.SensorsClient.bind(fake, loaded);
  await client.setLimit(speed(1), speed(2));
  await client.setLimit(speed(1), speed(2), { bound: 7 });
  await client.span();
  await client.span({ bound: 9 });
  const signal = new AbortController().signal;
  await client.setLimit(speed(1), speed(2), { signal });
  assert.deepEqual(
    fake.calls.map((call) => (call.opts as { bound: number }).bound),
    [50, 7, 100, 9, 50],
  );
  assert.equal((fake.calls[4].opts as { signal?: AbortSignal }).signal, signal);
});

Deno.test("bind resolves each payload type once and a call resolves none", async () => {
  const { loaded, resolved } = scripted(accept);
  const fake = port();
  fake.onQuery = () =>
    Promise.resolve(
      codec.Sensors.span.reply.encode({ min: speed(1), max: speed(2) }),
    );
  const client = codec.SensorsClient.bind(fake, loaded);
  assert.deepEqual(
    [...resolved].sort(),
    [
      codec.Sensors.setLimit.request.type,
      codec.Sensors.profile.request.type,
      codec.Sensors.profile.reply.type,
      codec.Sensors.span.request.type,
      codec.Sensors.span.reply.type,
    ].sort(),
  );
  const before = resolved.length;
  await client.span();
  await client.setLimit(speed(1), speed(2));
  assert.equal(resolved.length, before);
});

Deno.test("bind throws CatalogMismatch for the hash of the port and for the hash of the codec", () => {
  const { loaded } = scripted(accept);
  const other = codec.CATALOG_HASH.slice();
  other[0] ^= 0xff;
  assert.throws(
    () => codec.SensorsClient.bind(new FakePort(other), loaded),
    (error: unknown) =>
      error instanceof CatalogMismatch &&
      error.expected ===
        Array.from(codec.CATALOG_HASH, (b) => b.toString(16).padStart(2, "0"))
          .join("") &&
      error.message.includes("the port"),
  );
  assert.throws(
    () => codec.SensorsClient.bind(port(), { ...loaded, catalogHash: other }),
    (error: unknown) =>
      error instanceof CatalogMismatch &&
      error.message.includes("the wasm module"),
  );
  // A mismatching port is refused before the codec resolves any name.
  const { loaded: untouched, resolved } = scripted(accept);
  assert.throws(
    () => codec.SensorsClient.bind(new FakePort(other), untouched),
    CatalogMismatch,
  );
  assert.equal(resolved.length, 0);
});

Deno.test("the signature takes the declared parameters, in order, and refuses other types", () => {
  const { loaded } = scripted(accept);
  const client = codec.SensorsClient.bind(port(), loaded);
  // Never called: only the type checker reads these lines.
  const unused = () => {
    // @ts-expect-error a plain number is not a `Speed`
    client.setLimit(1, speed(2));
    // @ts-expect-error the second parameter is missing
    client.setLimit(speed(1));
    // @ts-expect-error `bound` must be a number
    client.span({ bound: "50" });
  };
  assert.equal(typeof unused, "function");
});
