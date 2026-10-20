import assert from "node:assert/strict";
import { connect, Kind } from "../mod.ts";
import type { Note, Occurrence, Sample } from "../mod.ts";
import * as catalog from "./catalog.ts";
import { startServer } from "./server.ts";

/** Resolves with the first value `accept` takes, or rejects after `ms`. */
function waiter<T>(what: string, ms: number) {
  let resolve!: (value: T) => void;
  let reject!: (e: Error) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  const timer = setTimeout(
    () => reject(new Error(`no ${what} within ${ms} ms`)),
    ms,
  );
  return {
    promise,
    accept(value: T) {
      clearTimeout(timer);
      resolve(value);
    },
    cancel() {
      clearTimeout(timer);
    },
  };
}

Deno.test("a Deno program receives a live signal from the Rust server", async () => {
  const server = await startServer();
  const notes: Note[] = [];
  const init = waiter<Sample>("init publish", 2000);
  const live = waiter<Sample>("live publish", 2000);
  const occurred = waiter<Occurrence>("occurrence", 2000);
  try {
    const session = await connect(`ws://127.0.0.1:${server.port}`, {
      catalog: { name: catalog.NAME, hash: catalog.HASH },
      encoding: catalog.ENCODING,
      onPublish(target, sample) {
        if (target.iface !== catalog.IFACE || target.ord !== catalog.SIGNAL) {
          return;
        }
        if (sample.provenance === "init") init.accept(sample);
        if (sample.provenance === "live") live.accept(sample);
      },
      onOccurrence(occurrence) {
        if (
          occurrence.iface === catalog.IFACE &&
          occurrence.ord === catalog.EVENT
        ) {
          occurred.accept(occurrence);
        }
      },
      onNote(note) {
        notes.push(note);
      },
    });

    const fixed = await session.read({
      kind: Kind.Fixed,
      iface: catalog.IFACE,
      ord: catalog.FIXED,
    });
    assert.equal(fixed.provenance, "live");
    assert.deepEqual(fixed.payload, catalog.FIXED_VALUE);
    assert.equal(fixed.envelope.seq, 0n);

    await session.subscribe({
      kind: Kind.Signal,
      iface: catalog.IFACE,
      ord: catalog.SIGNAL,
    });
    const held = await init.promise;
    assert.equal(held.envelope.seq, 0n);
    assert.equal(held.payload, null);
    assert.deepEqual(session.sample(catalog.IFACE, catalog.SIGNAL), held);

    await session.subscribe({
      kind: Kind.Event,
      iface: catalog.IFACE,
      ord: catalog.EVENT,
    });

    const args = Uint8Array.of(0x2a, 0x07);
    await session.command(catalog.IFACE, catalog.COMMAND, args, {
      bound: 2000,
    });

    const sample = await live.promise;
    assert.deepEqual(sample.payload, args);
    assert.ok(sample.envelope.seq >= 1n, `seq ${sample.envelope.seq}`);

    const occurrence = await occurred.promise;
    assert.deepEqual(occurrence.payload, args);

    const reply = await session.query(
      catalog.IFACE,
      catalog.QUERY,
      Uint8Array.of(0x01, 0x80, 0xff),
      { bound: 2000 },
    );
    assert.deepEqual(reply, Uint8Array.of(0x02, 0x81, 0x00));

    session.close();
    const ended = await session.closed;
    assert.equal(ended.code, 1000);
    assert.deepEqual(notes.filter((n) => n.note === "violation"), []);
  } finally {
    init.cancel();
    live.cancel();
    occurred.cancel();
    await server.stop();
  }
});
