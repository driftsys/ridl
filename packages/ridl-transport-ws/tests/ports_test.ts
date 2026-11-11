import assert from "node:assert/strict";
import { CallError } from "@driftsys/ridl-runtime";
import type {
  Caller,
  EventSource,
  SignalReader,
  StreamCaller,
} from "@driftsys/ridl-runtime";
import { connect, decode, Kind, SessionError, SUBPROTOCOL_V3 } from "../mod.ts";
import type {
  ConnectOptions,
  Frame,
  Note,
  Occurrence,
  Sample,
  Session,
  Target,
} from "../mod.ts";
import { FakeSocket } from "./fake_socket.ts";

const HASH = Uint8Array.from({ length: 32 }, (_, i) => i);
const SIGNAL: Target = { kind: Kind.Signal, iface: 1, ord: 2 };
const EVENT: Target = { kind: Kind.Event, iface: 1, ord: 3 };
const COMMAND: Target = { kind: Kind.Command, iface: 1, ord: 6 };
const QUERY: Target = { kind: Kind.Query, iface: 1, ord: 7 };
const BOUND = { bound: 20 };

const TABLE = {
  iface: 1,
  temperature: { iface: 1, ord: 2, kind: "signal" },
  warning: { iface: 1, ord: 3, kind: "event" },
  setLevel: { iface: 1, ord: 6, kind: "command" },
  average: { iface: 1, ord: 7, kind: "query" },
} as const;

// Every test turns the op sanitizer on, so a call's timer left running when a
// test ends fails that test.
function test(name: string, fn: () => Promise<void>): void {
  Deno.test({ name, sanitizeOps: true, fn });
}

async function attached(extra: Partial<ConnectOptions> = {}) {
  const sockets: FakeSocket[] = [];
  const notes: Note[] = [];
  const pending = connect("ws://127.0.0.1:9000", {
    catalog: { name: "veh.cabin", hash: HASH },
    interfaces: [TABLE],
    onNote: (n) => notes.push(n),
    webSocket: (url, protocols) => {
      const socket = new FakeSocket(url, protocols);
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
    ...extra,
  });
  const socket = sockets[0];
  socket.open(SUBPROTOCOL_V3);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  const session: Session = await pending;
  return { socket, session, notes };
}

function frames(socket: FakeSocket): Frame[] {
  return socket.sent.map((bytes) => {
    const d = decode(bytes);
    assert.ok(d.ok, "the session sent a message that does not decode");
    return d.frame;
  });
}

function accepted(target: Target): Frame {
  return { form: "answer", target, outcome: { outcome: "accepted" } };
}

function live(seq: bigint, payload: number[]): Frame {
  return {
    form: "publish",
    target: SIGNAL,
    envelope: { stamp: 100n + seq, seq },
    state: { provenance: "live", payload: Uint8Array.from(payload) },
  };
}

function occurrence(seq: bigint, payload: number[]): Frame {
  return {
    form: "occurrence",
    target: EVENT,
    envelope: { stamp: 200n + seq, seq },
    payload: Uint8Array.from(payload),
  };
}

/** The `response` that completes call 1 of a command or a query. */
function answerFor(target: Target): Frame {
  return target.kind === Kind.Query
    ? {
      form: "response",
      target,
      envelope: { stamp: 500n, seq: 1n },
      correlation: 1n,
      outcome: { outcome: "reply", payload: Uint8Array.from([9]) },
    }
    : accept(target, 1n);
}

function accept(target: Target, correlation: bigint): Frame {
  return {
    form: "response",
    target,
    envelope: { stamp: 500n, seq: 1n },
    correlation,
    outcome: { outcome: "accepted" },
  };
}

test("a Session is assignable to each of the four ports and answers a call of each", async () => {
  const { socket, session } = await attached();
  const caller: Caller = session;
  const reader: SignalReader = session;
  const source: EventSource = session;
  const streams: StreamCaller = session;
  assert.equal(streams, session);

  const command = caller.command(1, 6, new Uint8Array(), BOUND);
  socket.deliver(accept(COMMAND, 1n));
  assert.equal(await command, undefined);

  const subscribed = reader.subscribe(1, 2);
  socket.deliver(accepted(SIGNAL));
  await subscribed;
  assert.equal(reader.sample(1, 2), undefined);
  socket.deliver(live(1n, [7]));
  assert.deepEqual(reader.sample(1, 2)?.payload, Uint8Array.from([7]));
  reader.checkWith(1, 2, () => undefined);
  reader.unsubscribe(1, 2);
  assert.equal(reader.sample(1, 2), undefined);

  const events = source.subscribe(1, 3);
  socket.deliver(accepted(EVENT));
  await events;
  source.unsubscribe(1, 3);
});

test("catalogHash is the hash the session was connected with", async () => {
  const { session } = await attached();
  assert.deepEqual(session.catalogHash, HASH);
  assert.equal(session.catalogHash.length, 32);
});

test("read with (iface, ord) takes its kind from the tables", async () => {
  const { socket, session } = await attached();
  const read = session.read(1, 2);
  assert.deepEqual(frames(socket).at(-1), { form: "read", target: SIGNAL });
  socket.deliver(live(1n, [3]));
  assert.deepEqual((await read).payload, Uint8Array.from([3]));
});

test("subscribe with (iface, ord) sends the target the tables name", async () => {
  const { socket, session } = await attached();
  const subscribed = session.subscribe(1, 3, { bound: 20 });
  assert.deepEqual(frames(socket).at(-1), {
    form: "subscribe",
    target: EVENT,
  });
  socket.deliver(accepted(EVENT));
  await subscribed;
});

test("an (iface, ord) the tables do not hold is a RangeError and sends nothing", async () => {
  const { socket, session } = await attached();
  const before = socket.sent.length;
  await assert.rejects(session.subscribe(1, 99), RangeError);
  await assert.rejects(session.read(9, 2), RangeError);
  assert.throws(() => session.unsubscribe(1, 99), RangeError);
  assert.equal(socket.sent.length, before);
});

test("without tables, (iface, ord) is a RangeError and a Target still works", async () => {
  const { socket, session } = await attached({ interfaces: undefined });
  await assert.rejects(session.subscribe(1, 2), RangeError);
  const subscribed = session.subscribe(SIGNAL);
  socket.deliver(accepted(SIGNAL));
  await subscribed;
});

test("watch delivers every publish of its signal until it is removed, and onPublish still sees all", async () => {
  const publishes: Sample[] = [];
  const { socket, session } = await attached({
    onPublish: (_, sample) => publishes.push(sample),
  });
  const subscribed = session.subscribe(1, 2);
  socket.deliver(accepted(SIGNAL));
  await subscribed;
  const seen: Sample[] = [];
  const other: Sample[] = [];
  const remove = session.watch(1, 2, (s) => seen.push(s));
  session.watch(1, 2, (s) => other.push(s));
  socket.deliver(live(1n, [1]));
  remove();
  socket.deliver(live(2n, [2]));
  assert.equal(seen.length, 1);
  assert.deepEqual(seen[0].payload, Uint8Array.from([1]));
  assert.equal(other.length, 2);
  assert.equal(publishes.length, 2);
  remove();
});

test("a watch listener that removes itself or another stops the delivery to it", async () => {
  const { socket, session } = await attached();
  const subscribed = session.subscribe(1, 2);
  socket.deliver(accepted(SIGNAL));
  await subscribed;
  const calls: string[] = [];
  let removeSecond = () => {};
  session.watch(1, 2, () => {
    calls.push("first");
    removeSecond();
  });
  removeSecond = session.watch(1, 2, () => calls.push("second"));
  socket.deliver(live(1n, [1]));
  assert.deepEqual(calls, ["first"]);
});

test("watch does not deliver the publish of another signal", async () => {
  const { socket, session } = await attached();
  const seen: Sample[] = [];
  session.watch(1, 9, (s) => seen.push(s));
  const subscribed = session.subscribe(1, 2);
  socket.deliver(accepted(SIGNAL));
  await subscribed;
  socket.deliver(live(1n, [1]));
  assert.deepEqual(seen, []);
});

test("listen delivers every occurrence of its interface until it is removed, and onOccurrence still sees all", async () => {
  const occurrences: Occurrence[] = [];
  const { socket, session } = await attached({
    onOccurrence: (o) => occurrences.push(o),
  });
  const subscribed = session.subscribe(1, 3);
  socket.deliver(accepted(EVENT));
  await subscribed;
  const seen: Occurrence[] = [];
  const remove = session.listen(1, (o) => seen.push(o));
  const elsewhere: Occurrence[] = [];
  session.listen(2, (o) => elsewhere.push(o));
  socket.deliver(occurrence(1n, [1]));
  remove();
  socket.deliver(occurrence(2n, [2]));
  assert.equal(seen.length, 1);
  assert.deepEqual(seen[0].payload, Uint8Array.from([1]));
  assert.equal(occurrences.length, 2);
  assert.deepEqual(elsewhere, []);
});

test("listen delivers an occurrence over the limit empty, with detected set", async () => {
  const { socket, session } = await attached();
  const subscribed = session.subscribe(1, 3);
  socket.deliver(accepted(EVENT));
  await subscribed;
  const seen: Occurrence[] = [];
  session.listen(1, (o) => seen.push(o));
  socket.deliver({
    form: "occurrence",
    target: EVENT,
    envelope: { stamp: 201n, seq: 1n },
    payload: new Uint8Array(65_281),
  });
  assert.equal(seen.length, 1);
  assert.equal(seen[0].detected, "corrupt");
  assert.equal(seen[0].payload.length, 0);
});

test("watch and listen reject an invalid number", async () => {
  const { session } = await attached();
  assert.throws(() => session.watch(0, 2, () => {}), RangeError);
  assert.throws(() => session.listen(0, () => {}), RangeError);
});

for (
  const [name, target, call] of [
    [
      "command",
      COMMAND,
      (s: Session, signal: AbortSignal) =>
        s.command(1, 6, new Uint8Array(), { bound: 20, signal }),
    ],
    [
      "query",
      QUERY,
      (s: Session, signal: AbortSignal) =>
        s.query(1, 7, new Uint8Array(), { bound: 20, signal }),
    ],
  ] as const
) {
  test(`${name}: a signal aborted before the call rejects with its reason and sends nothing`, async () => {
    const { socket, session } = await attached();
    const reason = new Error("stop");
    const controller = new AbortController();
    controller.abort(reason);
    const before = socket.sent.length;
    await assert.rejects(call(session, controller.signal), (e) => e === reason);
    assert.equal(socket.sent.length, before);
    // The call took no number: the next call is still the first.
    const next = session.command(1, 6, new Uint8Array(), BOUND);
    assert.deepEqual(
      (frames(socket).at(-1) as { envelope: { seq: bigint } })
        .envelope.seq,
      1n,
    );
    socket.deliver(accept(COMMAND, 1n));
    await next;
  });

  test(`${name}: an abort after the send rejects with the reason, keeps the call pending and consumes the late answer`, async () => {
    const { socket, session, notes } = await attached();
    const reason = new Error("stop");
    const controller = new AbortController();
    const before = socket.sent.length;
    const pending = call(session, controller.signal);
    assert.equal(socket.sent.length, before + 1);
    controller.abort(reason);
    await assert.rejects(pending, (e) => e === reason);
    // The late answer names a call that is still in flight: no discard.
    const answer = answerFor(target);
    socket.deliver(answer);
    assert.equal(notes.length, 0);
    // A second answer for the same number is now unknown.
    socket.deliver(answer);
    assert.equal(notes.length, 1);
    assert.equal(
      notes[0].note === "discarded" && notes[0].reason,
      "unknown-correlation",
    );
  });

  test(`${name}: a signal that never aborts leaves the call to its answer`, async () => {
    const { socket, session } = await attached();
    const controller = new AbortController();
    const pending = call(session, controller.signal);
    if (name === "query") {
      socket.deliver({
        form: "response",
        target,
        envelope: { stamp: 500n, seq: 1n },
        correlation: 1n,
        outcome: { outcome: "reply", payload: Uint8Array.from([9]) },
      });
      assert.deepEqual(await pending, Uint8Array.from([9]));
    } else {
      socket.deliver(accept(target, 1n));
      assert.equal(await pending, undefined);
    }
    controller.abort();
  });
}

test("an invalid bound is a RangeError even when the signal is aborted", async () => {
  const { session } = await attached();
  const controller = new AbortController();
  controller.abort();
  await assert.rejects(
    session.command(1, 6, new Uint8Array(), {
      bound: 0,
      signal: controller.signal,
    }),
    RangeError,
  );
});

Deno.test("a SessionError is a CallError and a SessionError, with its own message", () => {
  const error = new SessionError({ category: "timeout" });
  assert.ok(error instanceof SessionError);
  assert.ok(error instanceof CallError);
  assert.ok(error instanceof Error);
  assert.equal(error.name, "SessionError");
  assert.equal(error.message, "the operation failed: timeout");
  assert.deepEqual(error.failure, { category: "timeout" });
  const contract = new SessionError({
    category: "contract",
    contract: { category: "invalid-value", rule: "range" },
  });
  assert.equal(
    contract.message,
    "the operation failed: contract invalid-value",
  );
});

test("a call that the session fails rejects with a CallError", async () => {
  const { socket, session } = await attached();
  const pending = session.command(1, 6, new Uint8Array(), BOUND);
  socket.serverClose(1006);
  await assert.rejects(
    pending,
    (e) => e instanceof CallError && e.failure.category === "down",
  );
});

test("catalogHash is a copy: neither the connect argument nor a returned value changes it", async () => {
  const hash = Uint8Array.from(HASH);
  const { session } = await attached({ catalog: { name: "veh.cabin", hash } });
  hash[0] = 99;
  assert.deepEqual(session.catalogHash, HASH);
  const first = session.catalogHash;
  first[1] = 99;
  assert.deepEqual(session.catalogHash, HASH);
});

test("a listen listener that removes itself or another stops the delivery to it", async () => {
  const { socket, session } = await attached();
  const subscribed = session.subscribe(1, 3);
  socket.deliver(accepted(EVENT));
  await subscribed;
  const calls: string[] = [];
  let removeSecond = () => {};
  session.listen(1, () => {
    calls.push("first");
    removeSecond();
  });
  removeSecond = session.listen(1, () => calls.push("second"));
  socket.deliver(occurrence(1n, [1]));
  assert.deepEqual(calls, ["first"]);
});

test("the same function registered twice is called twice, and a stale remover keeps a later registration", async () => {
  const { socket, session } = await attached();
  let subscribed = session.subscribe(1, 2);
  socket.deliver(accepted(SIGNAL));
  await subscribed;
  subscribed = session.subscribe(1, 3);
  socket.deliver(accepted(EVENT));
  await subscribed;
  let watched = 0;
  const watcher = () => watched++;
  session.watch(1, 2, watcher);
  const removeWatch = session.watch(1, 2, watcher);
  socket.deliver(live(1n, [1]));
  assert.equal(watched, 2);
  removeWatch();
  socket.deliver(live(2n, [2]));
  assert.equal(watched, 3);
  let heard = 0;
  const listener = () => heard++;
  const removeOld = session.listen(1, listener);
  removeOld();
  session.listen(1, listener);
  removeOld();
  socket.deliver(occurrence(1n, [1]));
  assert.equal(heard, 1);
});

test("a bound in the (iface, ord, opts) form of subscribe reaches the call", async () => {
  const { session } = await attached();
  await assert.rejects(
    session.subscribe(1, 2, { bound: 5 }),
    (e) => e instanceof SessionError && e.failure.category === "timeout",
  );
});

test("a signal in the (iface, ord, opts) form of read aborts it", async () => {
  const { session } = await attached();
  const reason = new Error("stop");
  const controller = new AbortController();
  const pending = session.read(1, 2, { signal: controller.signal });
  controller.abort(reason);
  await assert.rejects(pending, (e) => e === reason);
});

test("the abort listener is removed from the signal once the call settles", async () => {
  const { socket, session } = await attached();
  const signal = new AbortController().signal;
  let added = 0;
  let removed = 0;
  const add = signal.addEventListener.bind(signal);
  const remove = signal.removeEventListener.bind(signal);
  signal.addEventListener = (...a: Parameters<typeof add>) => {
    if (a[0] === "abort") added++;
    return add(...a);
  };
  signal.removeEventListener = (...a: Parameters<typeof remove>) => {
    if (a[0] === "abort") removed++;
    return remove(...a);
  };
  const pending = session.command(1, 6, new Uint8Array(), {
    bound: 20,
    signal,
  });
  socket.deliver(accept(COMMAND, 1n));
  await pending;
  assert.ok(added >= 1);
  assert.equal(removed, added);
});
