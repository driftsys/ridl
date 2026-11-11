import assert from "node:assert/strict";
import type {
  Caller,
  Codec,
  RawSample,
  SignalCheck,
  SignalReader,
} from "@driftsys/ridl-runtime";
import * as codec from "./bare.gauge.codec.ts";
import * as t from "./bare.gauge.ts";

Deno.test("a package with a signal and no string uses the runtime error", () => {
  assert.equal(codec.Gauge.level.kind, "signal");
  assert.equal(typeof codec.signalChecks, "function");
});

/** The one port that `GaugeClient` needs: it implements only `SignalReader`. */
class OnlyReader implements SignalReader {
  readonly catalogHash = codec.CATALOG_HASH.slice();
  readonly checks: SignalCheck[] = [];
  held: RawSample | undefined;
  checkWith(_iface: number, _ord: number, check: SignalCheck): void {
    this.checks.push(check);
  }
  subscribe(): Promise<void> {
    return Promise.resolve();
  }
  unsubscribe(): void {}
  sample(): RawSample | undefined {
    return this.held;
  }
  read(): Promise<RawSample> {
    return Promise.reject(new Error("not scripted"));
  }
  watch(): () => void {
    return () => {};
  }
}

const loaded: Codec = {
  catalogHash: codec.CATALOG_HASH.slice(),
  typeId: () => 1,
  verify: () => ({ ok: true }),
};

Deno.test("a signal-only client binds over a port that implements only SignalReader", () => {
  const reader = new OnlyReader();
  const client = codec.GaugeClient.bind(reader, loaded);
  assert.equal(reader.checks.length, 1);
  assert.deepEqual(client.level(), {
    provenance: "init",
    envelope: { stamp: 0n, seq: 0n },
    value: t.initLevel(),
  });
  const bytes = codec.Gauge.level.payload.encode({ value: 42 as t.Percent });
  reader.held = {
    provenance: "live",
    envelope: { stamp: 1n, seq: 2n },
    payload: bytes,
    lastGood: bytes,
  };
  assert.deepEqual(client.level().value, { value: 42 });
});

Deno.test("a port without SignalReader is refused by the type of bind", () => {
  // Never called: only the type checker reads these lines.
  const unused = (caller: Caller) => {
    // @ts-expect-error a `Caller` does not implement `SignalReader`
    codec.GaugeClient.bind(caller, loaded);
    // @ts-expect-error an object with only a catalog hash is no port
    codec.GaugeClient.bind({ catalogHash: new Uint8Array(32) }, loaded);
  };
  assert.equal(typeof unused, "function");
});
