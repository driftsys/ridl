import {
  assert,
  assertEquals,
  assertInstanceOf,
  assertRejects,
  assertStrictEquals,
} from "@std/assert";
import {
  CallError,
  type RawOccurrence,
  type RawSample,
  type SignalReader,
} from "../mod.ts";
import { FakePort } from "./fake_port.ts";

const envelope = { stamp: 1n, seq: 1n };
const live = (n: number): RawSample => ({
  provenance: "live",
  envelope,
  payload: Uint8Array.of(n),
  lastGood: Uint8Array.of(n),
});

Deno.test("a_port_that_implements_only_signal_reader_is_accepted", () => {
  const onlySignals: SignalReader = {
    catalogHash: new Uint8Array(32),
    checkWith() {},
    subscribe: () => Promise.resolve(),
    unsubscribe() {},
    sample: () => undefined,
    read: () => Promise.reject(new Error("unused")),
    watch: () => () => {},
  };
  const need = (reader: SignalReader) => reader.catalogHash.length;
  assertEquals(need(onlySignals), 32);
  assertEquals(need(new FakePort()), 32);
});

Deno.test("call_error_is_an_error_and_carries_its_failure", () => {
  const failure = {
    category: "contract",
    contract: { category: "invalid-value", rule: "range" },
  } as const;
  const error = new CallError(failure);
  assertInstanceOf(error, Error);
  assertEquals(error.name, "CallError");
  assertStrictEquals(error.failure, failure);
  assertEquals(new CallError({ category: "down" }).failure, {
    category: "down",
  });
});

Deno.test("fake_records_calls_and_resolves_as_scripted", async () => {
  const port = new FakePort();
  port.onQuery = () => Promise.resolve(Uint8Array.of(9));
  await port.command(1, 2, Uint8Array.of(7), { bound: 50 });
  assertEquals(
    await port.query(1, 3, Uint8Array.of(), { bound: 50 }),
    Uint8Array.of(9),
  );
  assertEquals(port.calls.map((c) => [c.method, c.iface, c.ord]), [
    ["command", 1, 2],
    ["query", 1, 3],
  ]);
  port.onCommand = () => Promise.reject(new CallError({ category: "busy" }));
  const error = await assertRejects(
    () => port.command(1, 2, Uint8Array.of(), { bound: 50 }),
    CallError,
  );
  assertEquals(error.failure, { category: "busy" });
});

Deno.test("watch_removal_stops_delivery", () => {
  const port = new FakePort();
  const seen: number[] = [];
  const remove = port.watch(1, 1, (s) => seen.push(s.payload![0]));
  port.publish(1, 1, live(1));
  remove();
  port.publish(1, 1, live(2));
  assertEquals(seen, [1]);
  assertEquals(port.watcherCount(1, 1), 0);
  assertEquals(port.sample(1, 1)?.payload, Uint8Array.of(2));
});

Deno.test("watch_is_per_signal", () => {
  const port = new FakePort();
  const seen: number[] = [];
  port.watch(1, 1, (s) => seen.push(s.payload![0]));
  port.publish(1, 2, live(5));
  assertEquals(seen, []);
});

Deno.test("listen_removal_stops_delivery", () => {
  const port = new FakePort();
  const seen: bigint[] = [];
  const occurrence = (seq: bigint): RawOccurrence => ({
    iface: 3,
    ord: 1,
    envelope: { stamp: 0n, seq },
    payload: new Uint8Array(),
  });
  const remove = port.listen(3, (o) => seen.push(o.envelope.seq));
  port.occur(occurrence(1n));
  remove();
  port.occur(occurrence(2n));
  assertEquals(seen, [1n]);
});

Deno.test("read_waits_for_the_next_publish", async () => {
  const port = new FakePort();
  port.publish(1, 1, live(1));
  let done = false;
  const read = port.read(1, 1).then((s) => {
    done = true;
    return s;
  });
  await Promise.resolve();
  assert(!done, "read must not resolve with the held sample");
  port.publish(1, 1, live(2));
  assertEquals((await read).payload, Uint8Array.of(2));
});

Deno.test("abort_rejects_with_the_signal_reason", async () => {
  const port = new FakePort();
  port.onQuery = () => new Promise(() => {});
  const reason = new Error("stop");
  const controller = new AbortController();
  const query = port.query(1, 1, Uint8Array.of(), {
    bound: 50,
    signal: controller.signal,
  });
  controller.abort(reason);
  assertStrictEquals(await assertRejects(() => query), reason);

  const read = port.read(1, 1, { signal: AbortSignal.abort(reason) });
  assertStrictEquals(await assertRejects(() => read), reason);

  const command = port.command(1, 1, Uint8Array.of(), {
    bound: 50,
    signal: AbortSignal.abort(reason),
  });
  assertStrictEquals(await assertRejects(() => command), reason);
});
