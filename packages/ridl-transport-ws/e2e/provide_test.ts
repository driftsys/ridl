import assert from "node:assert/strict";
import { connect, decode, Kind, provide, SessionError } from "../mod.ts";
import type { Provision, Sample, Session } from "../mod.ts";
import { waiter } from "./waiter.ts";

// The committed identity fixture: alpha.First/Unit (1), alpha.Face (1),
// value (signal 1), changed (event 2), put (command 3), get (query 4).
const catalog = {
  name: "alpha",
  hash: new Uint8Array([
    14,
    60,
    221,
    186,
    156,
    205,
    215,
    219,
    138,
    114,
    25,
    248,
    25,
    135,
    168,
    28,
    142,
    146,
    39,
    76,
    181,
    65,
    141,
    187,
    106,
    210,
    72,
    117,
    79,
    48,
    188,
    186,
  ]),
};
const signal = { kind: Kind.Signal, iface: 1, ord: 1 };
const event = { kind: Kind.Event, iface: 1, ord: 2 };
const members = [
  { iface: 1, ord: 1, kind: "signal" },
  { iface: 1, ord: 2, kind: "event" },
  { iface: 1, ord: 3, kind: "command" },
  { iface: 1, ord: 4, kind: "query" },
] as const;
async function bounded<T>(p: Promise<T>, what: string): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try {
    return await Promise.race([
      p,
      new Promise<never>((_, reject) => {
        timer = setTimeout(() => reject(Error(`timeout: ${what}`)), 5000);
      }),
    ]);
  } finally {
    clearTimeout(timer);
  }
}

Deno.test("provide_replaces_detached_instance_end_to_end", async () => {
  const path = Deno.env.get("RIDL_DEBUG_GATEWAY_FIXTURE");
  assert(path, "RIDL_DEBUG_GATEWAY_FIXTURE must name debug_gateway_fixture");
  const child = new Deno.Command(path, {
    stdin: "piped",
    stdout: "piped",
    stderr: "inherit",
  }).spawn();
  const input = child.stdin.getWriter();
  const reader = child.stdout.pipeThrough(new TextDecoderStream()).getReader();
  let buffered = "";
  async function line(): Promise<string> {
    while (!buffered.includes("\n")) {
      const next = await reader.read();
      assert(!next.done, "fixture exited before response");
      buffered += next.value;
    }
    const end = buffered.indexOf("\n");
    const result = buffered.slice(0, end);
    buffered = buffered.slice(end + 1);
    return result;
  }
  async function control(command: string, expected: string) {
    await input.write(new TextEncoder().encode(command + "\n"));
    assert.equal(await bounded(line(), command), expected);
  }
  let provider: Provision | undefined;
  let listening: Session | undefined;
  const live = waiter<Sample>("listening live sample", 3000);
  const invalid = waiter<Sample>("listening invalid sample", 3000);
  const occurred = waiter<Uint8Array>("listening occurrence", 3000);
  try {
    const port = /^port (\d+)$/.exec(await bounded(line(), "listening port"));
    const inject = /^inject-port (\d+)$/.exec(
      await bounded(line(), "injection port"),
    );
    assert(port && inject);
    listening = await bounded(
      connect(`ws://127.0.0.1:${port[1]}`, {
        catalog,
        encoding: "flatbuffers",
        onPublish(t, s) {
          assert.deepEqual(t, signal);
          if (s.provenance === "live") live.accept(s);
          if (s.provenance === "invalid") invalid.accept(s);
        },
        onOccurrence(o) {
          assert.equal(o.iface, 1);
          assert.equal(o.ord, 2);
          if (o.payload instanceof Uint8Array) occurred.accept(o.payload);
        },
      }),
      "listening attach",
    );
    await bounded(listening.subscribe(signal), "signal subscription");
    await bounded(listening.subscribe(event), "event subscription");
    const requests: { ord: number; payload: Uint8Array }[] = [];
    // Observe the real subscribe messages after the provider processes them.
    const subscribed = waiter<void>("provider subscriptions", 3000);
    const seen = new Set<number>();
    provider = await bounded(
      provide(`ws://127.0.0.1:${inject[1]}/alpha.First/Unit/alpha`, {
        catalog,
        encoding: "flatbuffers",
        members,
        onRequest(r) {
          assert.equal(r.target.iface, 1);
          requests.push({ ord: r.target.ord, payload: r.payload });
          if (r.target.kind === Kind.Command) {
            assert.equal(r.target.ord, 3);
            return { outcome: "accepted" };
          }
          assert.equal(r.target.kind, Kind.Query);
          assert.equal(r.target.ord, 4);
          return { outcome: "reply", payload: new Uint8Array([91]) };
        },
        webSocket(url, protocols) {
          const ws = new WebSocket(url, protocols);
          ws.addEventListener("message", (e) => {
            const frame = decode(new Uint8Array(e.data));
            if (frame.ok && frame.frame.form === "subscribe") {
              assert.equal(frame.frame.target.iface, 1);
              seen.add(frame.frame.target.ord);
              if (seen.has(1) && seen.has(2)) {
                // The listener precedes onmessage; the microtask follows dispatch.
                queueMicrotask(() => subscribed.accept());
              }
            }
          });
          return ws;
        },
      }),
      "provider attach",
    );
    subscribed.start();
    await subscribed.promise;
    await control("reservation", "blocked");
    live.start();
    provider.publish(1, 1, new Uint8Array([42]), 100n);
    assert.deepEqual((await live.promise).payload, new Uint8Array([42]));
    await control("signal", "signal 42");
    invalid.start();
    provider.invalidate(1, 1, 101n);
    const sample = await invalid.promise;
    assert.equal(sample.payload, null);
    assert.equal(sample.provenance, "invalid");
    assert.equal(sample.detected, undefined);
    await control("invalid", "invalid 42");
    occurred.start();
    provider.raise(1, 2, new Uint8Array([7]), 102n);
    assert.deepEqual(await occurred.promise, new Uint8Array([7]));
    await control("event", "event 7");
    await control("command", "command accepted");
    await control("query", "query 91");
    await bounded(
      listening.command(1, 3, new Uint8Array([12]), { bound: 2000 }),
      "listening command",
    );
    assert.deepEqual(requests, [
      { ord: 3, payload: new Uint8Array([11]) },
      { ord: 4, payload: new Uint8Array([13]) },
      { ord: 3, payload: new Uint8Array([12]) },
    ]);
    provider.close();
    await bounded(provider.closed, "provider close");
    assert.throws(
      () => provider!.raise(1, 2, new Uint8Array([8]), 103n),
      SessionError,
    );
    await control("released", "released");
  } finally {
    live.cancel();
    invalid.cancel();
    occurred.cancel();
    provider?.close();
    listening?.close();
    await input.close().catch(() => {});
    try {
      await bounded(child.status, "fixture exit");
    } catch {
      try {
        child.kill("SIGKILL");
      } catch { /* Already exited. */ }
      await child.status;
    }
    await reader.cancel().catch(() => {});
    if (listening) await bounded(listening.closed, "listening close");
  }
});
