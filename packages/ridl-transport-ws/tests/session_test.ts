import assert from "node:assert/strict";
import {
  AttachError,
  AttachRefused,
  connect,
  decode,
  encode,
  Kind,
  ProtocolError,
  SessionError,
  SUBPROTOCOL,
} from "../mod.ts";
import type {
  ConnectOptions,
  Frame,
  Note,
  Occurrence,
  PublishState,
  Sample,
  Session,
  Target,
} from "../mod.ts";
import { FakeSocket } from "./fake_socket.ts";

const HASH = Uint8Array.from({ length: 32 }, (_, i) => i);
const SIGNAL: Target = { kind: Kind.Signal, iface: 1, ord: 2 };
const EVENT: Target = { kind: Kind.Event, iface: 1, ord: 3 };
const FIXED: Target = { kind: Kind.Fixed, iface: 1, ord: 5 };

interface Harness {
  socket: FakeSocket;
  sockets: FakeSocket[];
  pending: Promise<Session>;
  notes: Note[];
  publishes: { target: Target; sample: Sample }[];
  occurrences: Occurrence[];
}

function start(name = "veh.cabin", extra: Partial<ConnectOptions> = {}) {
  const sockets: FakeSocket[] = [];
  const notes: Note[] = [];
  const publishes: { target: Target; sample: Sample }[] = [];
  const occurrences: Occurrence[] = [];
  const pending = connect("ws://127.0.0.1:9000", {
    catalog: { name, hash: HASH },
    onPublish: (target, sample) => publishes.push({ target, sample }),
    onOccurrence: (o) => occurrences.push(o),
    onNote: (n) => notes.push(n),
    webSocket: (url, protocols) => {
      const socket = new FakeSocket(url, protocols);
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
    ...extra,
  });
  return { sockets, pending, notes, publishes, occurrences };
}

async function attached(): Promise<Harness & { session: Session }> {
  const h = start();
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  const session = await h.pending;
  return { ...h, socket, session };
}

function frames(socket: FakeSocket): Frame[] {
  return socket.sent.map((bytes) => {
    const d = decode(bytes);
    assert.ok(d.ok, "the session sent a message that does not decode");
    return d.frame;
  });
}

function lastSent(socket: FakeSocket): Frame {
  const all = frames(socket);
  return all[all.length - 1];
}

function publish(target: Target, seq: bigint, state: PublishState): Frame {
  return {
    form: "publish",
    target,
    envelope: { stamp: 100n + seq, seq },
    state,
  };
}

function live(target: Target, seq: bigint, payload: number[]): Frame {
  return publish(target, seq, {
    provenance: "live",
    payload: Uint8Array.from(payload),
  });
}

function occurrence(target: Target, seq: bigint, payload: number[]): Frame {
  return {
    form: "occurrence",
    target,
    envelope: { stamp: 200n + seq, seq },
    payload: Uint8Array.from(payload),
  };
}

function accepted(target: Target): Frame {
  return { form: "answer", target, outcome: { outcome: "accepted" } };
}

function refusedAnswer(target: Target): Frame {
  return {
    form: "answer",
    target,
    outcome: {
      outcome: "contract",
      contract: { category: "unknown-interaction" },
    },
  };
}

function isFailure(category: string) {
  return (e: unknown) =>
    e instanceof SessionError && e.failure.category === category;
}

const unknownInteraction = (e: unknown) =>
  e instanceof SessionError &&
  e.failure.category === "contract" &&
  e.failure.contract.category === "unknown-interaction";

async function subscribed(h: Harness & { session: Session }, t: Target) {
  const done = h.session.subscribe(t);
  h.socket.deliver(accepted(t));
  await done;
}

Deno.test("connect sends attach with the catalog, frame version 1 and proto3", async () => {
  const h = start();
  assert.equal(h.sockets.length, 1);
  const socket = h.sockets[0];
  assert.equal(socket.url, "ws://127.0.0.1:9000");
  assert.deepEqual(socket.protocols, [SUBPROTOCOL]);
  assert.equal(socket.binaryType, "arraybuffer");
  assert.equal(socket.sent.length, 0);
  socket.open(SUBPROTOCOL);
  assert.deepEqual(frames(socket), [{
    form: "attach",
    frameVersion: 1,
    encoding: 2,
    hash: HASH,
    name: "veh.cabin",
  }]);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  await h.pending;
  assert.deepEqual(h.notes, []);
});

Deno.test("connect rejects with no-subprotocol and closes without a code when the server selected none", async () => {
  const h = start();
  const socket = h.sockets[0];
  socket.open("");
  await assert.rejects(
    h.pending,
    (e: unknown) => e instanceof ProtocolError && e.cause === "no-subprotocol",
  );
  assert.deepEqual(socket.closes, [{}]);
  assert.equal(socket.sent.length, 0);
  assert.deepEqual(h.notes, [{
    note: "violation",
    violation: "no-subprotocol",
  }]);
});

Deno.test("connect rejects with AttachRefused catalog-mismatch and closed reports the refusal", async () => {
  const h = start();
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  socket.deliver({
    form: "attached",
    outcome: { outcome: "refused", reason: "catalog-mismatch" },
  });
  let refusal: AttachRefused | undefined;
  await h.pending.catch((e) => {
    refusal = e;
  });
  assert.ok(refusal instanceof AttachRefused);
  assert.equal(refusal.reason, "catalog-mismatch");
  socket.serverClose(1000);
  assert.deepEqual(await refusal.closed, {
    code: 1000,
    reason: "",
    refused: "catalog-mismatch",
  });
});

Deno.test("connect rejects a 256-byte name and opens no socket", async () => {
  // 85 three-byte characters are 255 bytes: accepted.
  const ok = start("€".repeat(85));
  assert.equal(ok.sockets.length, 1);
  ok.sockets[0].open(SUBPROTOCOL);
  const sent = lastSent(ok.sockets[0]);
  assert.equal(sent.form === "attach" && sent.name, "€".repeat(85));

  for (const name of ["€".repeat(85) + "a", "a".repeat(256)]) {
    const h = start(name);
    await assert.rejects(
      h.pending,
      (e: unknown) => e instanceof AttachError && e.reason === "too-long",
    );
    assert.equal(h.sockets.length, 0);
  }
});

Deno.test("a first message other than attached is a violation that closes without a code", async () => {
  const h = start();
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  socket.deliver(accepted(SIGNAL));
  await assert.rejects(
    h.pending,
    (e: unknown) => e instanceof ProtocolError && e.cause === "unexpected-form",
  );
  assert.deepEqual(socket.closes, [{}]);
  assert.deepEqual(h.notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
});

Deno.test("a text message is a violation", async () => {
  const h = await attached();
  h.socket.deliver("hello");
  assert.deepEqual(h.socket.closes, [{}]);
  assert.deepEqual(h.notes, [{ note: "violation", violation: "text-message" }]);
  await assert.rejects(h.session.subscribe(SIGNAL), isFailure("down"));
  h.socket.serverClose(1005);
  assert.deepEqual(await h.session.closed, {
    code: 1005,
    reason: "",
    violation: "text-message",
  });
});

Deno.test("subscribe of a signal resolves on answer accepted, and the first publish is held", async () => {
  const h = await attached();
  const done = h.session.subscribe(SIGNAL);
  assert.deepEqual(lastSent(h.socket), { form: "subscribe", target: SIGNAL });
  h.socket.deliver(accepted(SIGNAL));
  await done;
  assert.equal(h.session.sample(1, 2), undefined);
  h.socket.deliver(publish(SIGNAL, 0n, { provenance: "init" }));
  const expected: Sample = {
    provenance: "init",
    envelope: { stamp: 100n, seq: 0n },
    payload: null,
  };
  assert.deepEqual(h.session.sample(1, 2), expected);
  assert.deepEqual(h.publishes, [{ target: SIGNAL, sample: expected }]);
  assert.deepEqual(h.notes, []);
});

Deno.test("subscribe rejects with contract unknown-interaction on a refusing answer", async () => {
  const h = await attached();
  const done = h.session.subscribe(SIGNAL);
  h.socket.deliver(refusedAnswer(SIGNAL));
  await assert.rejects(done, unknownInteraction);
  h.socket.deliver(live(SIGNAL, 1n, [1]));
  assert.equal(h.session.sample(1, 2), undefined);
  assert.deepEqual(h.notes.map((n) => n.note === "discarded" && n.reason), [
    "not-subscribed",
  ]);
});

Deno.test("a publish whose seq is below the held one is discarded as stale, and one equal to it is dropped", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  h.socket.deliver(live(SIGNAL, 3n, [1]));
  // The server reuses a seq only for an identical sample: a seq equal to the
  // held one is dropped and not recorded.
  h.socket.deliver(live(SIGNAL, 3n, [2]));
  const older = live(SIGNAL, 2n, [3]);
  h.socket.deliver(older);
  assert.deepEqual(h.session.sample(1, 2)?.payload, Uint8Array.from([1]));
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "stale", frame: older },
  ]);
  h.socket.deliver(live(SIGNAL, 4n, [4]));
  assert.deepEqual(h.session.sample(1, 2)?.payload, Uint8Array.from([4]));
  assert.equal(h.publishes.length, 2);
});

Deno.test("unsubscribe drops the held sample, and a later subscribe starts from the next publish", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  h.socket.deliver(live(SIGNAL, 5n, [5]));
  h.session.unsubscribe(SIGNAL);
  assert.deepEqual(lastSent(h.socket), { form: "unsubscribe", target: SIGNAL });
  assert.equal(h.session.sample(1, 2), undefined);
  const inFlight = live(SIGNAL, 6n, [6]);
  h.socket.deliver(inFlight);
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "not-subscribed", frame: inFlight },
  ]);
  await subscribed(h, SIGNAL);
  // A seq below the one held before the unsubscribe is accepted.
  h.socket.deliver(live(SIGNAL, 1n, [7]));
  assert.deepEqual(h.session.sample(1, 2)?.payload, Uint8Array.from([7]));
  assert.equal(h.session.sample(1, 2)?.envelope.seq, 1n);
});

Deno.test("read of an unsubscribed signal settles with the publish and holds nothing", async () => {
  const h = await attached();
  const reading = h.session.read(SIGNAL);
  assert.deepEqual(lastSent(h.socket), { form: "read", target: SIGNAL });
  h.socket.deliver(live(SIGNAL, 7n, [0xaa]));
  assert.deepEqual(await reading, {
    provenance: "live",
    envelope: { stamp: 107n, seq: 7n },
    payload: Uint8Array.from([0xaa]),
  });
  assert.equal(h.session.sample(1, 2), undefined);
  assert.deepEqual(h.publishes, []);
  assert.deepEqual(h.notes, []);
});

Deno.test("read of a subscribed signal settles with the publish and updates the held sample", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  h.socket.deliver(live(SIGNAL, 1n, [1]));
  const reading = h.session.read(SIGNAL);
  h.socket.deliver(live(SIGNAL, 2n, [2]));
  const sample = await reading;
  assert.equal(sample.envelope.seq, 2n);
  assert.deepEqual(h.session.sample(1, 2), sample);
  assert.equal(h.publishes.length, 2);
  assert.deepEqual(h.publishes[1], { target: SIGNAL, sample });
  assert.deepEqual(h.notes, []);
});

Deno.test("read of a fixed settles with a live publish whose seq is 0", async () => {
  const h = await attached();
  const reading = h.session.read(FIXED);
  assert.deepEqual(lastSent(h.socket), { form: "read", target: FIXED });
  h.socket.deliver(live(FIXED, 0n, [0xf0, 0x0d]));
  assert.deepEqual(await reading, {
    provenance: "live",
    envelope: { stamp: 100n, seq: 0n },
    payload: Uint8Array.from([0xf0, 0x0d]),
  });
  assert.equal(h.session.sample(1, 5), undefined);
  assert.deepEqual(h.publishes, []);
  assert.deepEqual(h.notes, []);
});

Deno.test("a refused read rejects with contract unknown-interaction", async () => {
  const h = await attached();
  const reading = h.session.read(EVENT);
  assert.deepEqual(lastSent(h.socket), { form: "read", target: EVENT });
  h.socket.deliver(refusedAnswer(EVENT));
  await assert.rejects(reading, unknownInteraction);
  assert.deepEqual(h.notes, []);
});

Deno.test("two reads of one signal settle with one publish", async () => {
  const h = await attached();
  const first = h.session.read(SIGNAL);
  const second = h.session.read(SIGNAL);
  h.socket.deliver(live(SIGNAL, 4n, [4]));
  const [a, b] = await Promise.all([first, second]);
  assert.equal(a.envelope.seq, 4n);
  assert.deepEqual(a, b);
  // The second publish has no read waiting for it.
  h.socket.deliver(live(SIGNAL, 5n, [5]));
  assert.deepEqual(h.notes.map((n) => n.note === "discarded" && n.reason), [
    "not-subscribed",
  ]);
});

Deno.test("an occurrence of a subscribed event is delivered with its payload", async () => {
  const h = await attached();
  await subscribed(h, EVENT);
  h.socket.deliver(occurrence(EVENT, 1n, [9, 8]));
  assert.deepEqual(h.occurrences, [{
    iface: 1,
    ord: 3,
    envelope: { stamp: 201n, seq: 1n },
    payload: Uint8Array.from([9, 8]),
  }]);
  assert.deepEqual(h.notes, []);
});

Deno.test("a gap in an event's seq is recorded as a loss and the occurrence is delivered", async () => {
  const h = await attached();
  await subscribed(h, EVENT);
  h.socket.deliver(occurrence(EVENT, 1n, [1]));
  // The smallest gap: one occurrence missing.
  h.socket.deliver(occurrence(EVENT, 3n, [3]));
  assert.deepEqual(h.notes, [
    { note: "loss", iface: 1, ord: 3, expected: 2n, received: 3n },
  ]);
  assert.deepEqual(h.occurrences.map((o) => o.envelope.seq), [1n, 3n]);
  const repeat = occurrence(EVENT, 3n, [3]);
  h.socket.deliver(repeat);
  assert.deepEqual(h.notes[1], {
    note: "discarded",
    reason: "stale",
    frame: repeat,
  });
  assert.equal(h.occurrences.length, 2);
  // The next seq in order records nothing.
  h.socket.deliver(occurrence(EVENT, 4n, [4]));
  assert.equal(h.occurrences.length, 3);
  assert.equal(h.notes.length, 2);
});

Deno.test("an occurrence of an unsubscribed event is discarded", async () => {
  const h = await attached();
  const frame = occurrence(EVENT, 1n, [1]);
  h.socket.deliver(frame);
  assert.equal(h.occurrences.length, 0);
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "not-subscribed", frame },
  ]);
  await subscribed(h, EVENT);
  h.socket.deliver(occurrence(EVENT, 5n, [5]));
  h.session.unsubscribe(EVENT);
  assert.deepEqual(lastSent(h.socket), { form: "unsubscribe", target: EVENT });
  const inFlight = occurrence(EVENT, 6n, [6]);
  h.socket.deliver(inFlight);
  assert.deepEqual(h.notes[1], {
    note: "discarded",
    reason: "not-subscribed",
    frame: inFlight,
  });
  // The unsubscribe dropped the last seq: seq 1 is neither stale nor a loss.
  await subscribed(h, EVENT);
  h.socket.deliver(frame);
  assert.deepEqual(h.occurrences.map((o) => o.envelope.seq), [5n, 1n]);
  assert.equal(h.notes.length, 2);
});

Deno.test("a publish with bytes after a header that carries no payload is discarded and the session continues", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  // An `Init` publish, which carries no payload, with one byte too many.
  const init = encode(publish(SIGNAL, 1n, { provenance: "init" }));
  const header = new Uint8Array(init.length + 1);
  header.set(init);
  header[init.length] = 0xee;
  h.socket.deliver(header);
  assert.deepEqual(h.notes, [{
    note: "discarded",
    reason: "unexpected-payload",
  }]);
  assert.deepEqual(h.socket.closes, []);
  assert.equal(h.session.sample(1, 2), undefined);
  h.socket.deliver(live(SIGNAL, 2n, [2]));
  assert.equal(h.session.sample(1, 2)?.envelope.seq, 2n);
});

Deno.test("close sends 1000 and closed resolves", async () => {
  const h = await attached();
  h.session.close();
  assert.deepEqual(h.socket.closes, [{ code: 1000 }]);
  await assert.rejects(h.session.read(SIGNAL), isFailure("down"));
  h.socket.serverClose(1000);
  assert.deepEqual(await h.session.closed, { code: 1000, reason: "" });
  assert.deepEqual(h.notes, []);
});

Deno.test("a server close rejects a waiting subscribe and read with down", async () => {
  const h = await attached();
  const subscribing = h.session.subscribe(SIGNAL);
  const reading = h.session.read(FIXED);
  const sent = h.socket.sent.length;
  h.socket.serverClose(1001, "going away");
  await assert.rejects(subscribing, isFailure("down"));
  await assert.rejects(reading, isFailure("down"));
  await assert.rejects(h.session.subscribe(EVENT), isFailure("down"));
  assert.equal(h.socket.sent.length, sent);
  assert.deepEqual(await h.session.closed, {
    code: 1001,
    reason: "going away",
  });
});

Deno.test("an answer with no subscribe waiting is discarded as unexpected-answer", async () => {
  const h = await attached();
  const none = accepted(SIGNAL);
  h.socket.deliver(none);
  // An accepted answer does not settle a read: a fulfilled read has no answer.
  const reading = h.session.read(SIGNAL);
  const forRead = accepted(SIGNAL);
  h.socket.deliver(forRead);
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "unexpected-answer", frame: none },
    { note: "discarded", reason: "unexpected-answer", frame: forRead },
  ]);
  h.socket.deliver(live(SIGNAL, 1n, [1]));
  assert.equal((await reading).envelope.seq, 1n);
  assert.equal(h.session.sample(1, 2), undefined);
});

Deno.test("a connection error rejects a waiting read with down and closed reports 1006", async () => {
  const h = await attached();
  const reading = h.session.read(SIGNAL);
  h.socket.error();
  await assert.rejects(reading, isFailure("down"));
  await assert.rejects(h.session.read(SIGNAL), isFailure("down"));
  h.socket.serverClose(1006);
  assert.deepEqual(await h.session.closed, { code: 1006, reason: "" });
});

/** Runs `body` with a handler that keeps reported errors from failing the test. */
async function catchingReported(
  body: (seen: unknown[]) => Promise<void>,
): Promise<void> {
  const seen: unknown[] = [];
  const handler = (e: ErrorEvent) => {
    e.preventDefault();
    seen.push(e.error);
  };
  globalThis.addEventListener("error", handler);
  try {
    await body(seen);
  } finally {
    globalThis.removeEventListener("error", handler);
  }
}

async function attachedWith(extra: Partial<ConnectOptions>) {
  const h = start("veh.cabin", extra);
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  return { ...h, socket, session: await h.pending };
}

Deno.test("an unsubscribe sent before the subscribe's answer leaves the signal unsubscribed", async () => {
  const h = await attached();
  const subscribing = h.session.subscribe(SIGNAL);
  h.session.unsubscribe(SIGNAL);
  h.socket.deliver(accepted(SIGNAL));
  await subscribing;
  const inFlight = live(SIGNAL, 1n, [1]);
  h.socket.deliver(inFlight);
  assert.equal(h.session.sample(1, 2), undefined);
  assert.deepEqual(h.publishes, []);
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "not-subscribed", frame: inFlight },
  ]);
});

Deno.test("an unsubscribe sent before the subscribe's answer leaves the event unsubscribed", async () => {
  const h = await attached();
  const subscribing = h.session.subscribe(EVENT);
  h.session.unsubscribe(EVENT);
  h.socket.deliver(accepted(EVENT));
  await subscribing;
  const frame = occurrence(EVENT, 1n, [1]);
  h.socket.deliver(frame);
  assert.deepEqual(h.occurrences, []);
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "not-subscribed", frame },
  ]);
});

Deno.test("a throwing onNote on a violation still closes without a code and rejects with down", async () => {
  await catchingReported(async (seen) => {
    const h = await attachedWith({
      onNote: () => {
        throw new Error("listener failed");
      },
    });
    const reading = h.session.read(SIGNAL);
    h.socket.deliver("hello");
    assert.deepEqual(h.socket.closes, [{}]);
    await assert.rejects(reading, isFailure("down"));
    await assert.rejects(h.session.read(SIGNAL), isFailure("down"));
    assert.equal(seen.length, 1);
    assert.equal((seen[0] as Error).message, "listener failed");
    h.socket.serverClose(1005);
    assert.deepEqual(await h.session.closed, {
      code: 1005,
      reason: "",
      violation: "text-message",
    });
  });
});

Deno.test("a throwing onNote on a loss records the loss once and delivers every occurrence", async () => {
  await catchingReported(async (seen) => {
    const notes: Note[] = [];
    const h = await attachedWith({
      onNote: (n) => {
        notes.push(n);
        throw new Error("listener failed");
      },
    });
    await subscribed(h, EVENT);
    for (const seq of [1n, 3n, 4n]) {
      h.socket.deliver(occurrence(EVENT, seq, [Number(seq)]));
    }
    assert.deepEqual(h.occurrences.map((o) => o.envelope.seq), [1n, 3n, 4n]);
    assert.deepEqual(notes, [
      { note: "loss", iface: 1, ord: 3, expected: 2n, received: 3n },
    ]);
    assert.equal(seen.length, 1);
  });
});

Deno.test("a throwing onPublish and onOccurrence do not stop the session updating its state", async () => {
  await catchingReported(async (seen) => {
    const notes: Note[] = [];
    const h = await attachedWith({
      onPublish: () => {
        throw new Error("publish listener failed");
      },
      onOccurrence: () => {
        throw new Error("occurrence listener failed");
      },
      onNote: (n) => notes.push(n),
    });
    await subscribed(h, SIGNAL);
    await subscribed(h, EVENT);
    h.socket.deliver(live(SIGNAL, 1n, [1]));
    h.socket.deliver(live(SIGNAL, 2n, [2]));
    assert.equal(h.session.sample(1, 2)?.envelope.seq, 2n);
    h.socket.deliver(occurrence(EVENT, 1n, [1]));
    h.socket.deliver(occurrence(EVENT, 2n, [2]));
    const repeat = occurrence(EVENT, 2n, [2]);
    h.socket.deliver(repeat);
    // seq 1 then 2 recorded no loss, and the repeat of 2 is stale.
    assert.deepEqual(notes, [
      { note: "discarded", reason: "stale", frame: repeat },
    ]);
    assert.equal(seen.length, 4);
  });
});

Deno.test("a second attached is a violation that closes without a code", async () => {
  const h = await attached();
  h.socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  assert.deepEqual(h.socket.closes, [{}]);
  assert.deepEqual(h.notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
});

Deno.test("a consumer-to-provider form from the server is a violation that closes without a code", async () => {
  const h = await attached();
  h.socket.deliver({ form: "subscribe", target: SIGNAL });
  assert.deepEqual(h.socket.closes, [{}]);
  assert.deepEqual(h.notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
});

Deno.test("a malformed binary message after attach is a violation", async () => {
  const h = await attached();
  const reading = h.session.read(SIGNAL);
  h.socket.deliver(new Uint8Array([0x07, 0x01, 1]));
  assert.deepEqual(h.socket.closes, [{}]);
  assert.deepEqual(h.notes, [{ note: "violation", violation: "too-short" }]);
  await assert.rejects(reading, isFailure("down"));
});

Deno.test("a stale publish that answers a read of a subscribed signal settles the read and leaves the held sample", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  h.socket.deliver(live(SIGNAL, 3n, [3]));
  const held = h.session.sample(1, 2);
  const reading = h.session.read(SIGNAL);
  h.socket.deliver(live(SIGNAL, 3n, [9]));
  const sample = await reading;
  assert.deepEqual(sample.payload, Uint8Array.from([9]));
  assert.equal(h.session.sample(1, 2), held);
  assert.equal(h.publishes.length, 1);
  assert.deepEqual(h.notes, []);
});

Deno.test("a server close or an error before attached rejects the pending connect with down", async () => {
  const closing = start();
  closing.sockets[0].open(SUBPROTOCOL);
  closing.sockets[0].serverClose(1011, "internal error");
  await assert.rejects(closing.pending, isFailure("down"));

  const failing = start();
  failing.sockets[0].error();
  await assert.rejects(failing.pending, isFailure("down"));
});

Deno.test("a fixed publish on a subscribed signal's numbers does not touch the held sample", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  h.socket.deliver(live(SIGNAL, 1n, [1]));
  const held = h.session.sample(1, 2);
  const fixed: Target = { kind: Kind.Fixed, iface: 1, ord: 2 };
  const unsolicited = live(fixed, 5n, [5]);
  h.socket.deliver(unsolicited);
  const reading = h.session.read(fixed);
  h.socket.deliver(live(fixed, 6n, [6]));
  assert.deepEqual((await reading).payload, Uint8Array.from([6]));
  assert.equal(h.session.sample(1, 2), held);
  assert.equal(h.publishes.length, 1);
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "not-subscribed", frame: unsolicited },
  ]);
});

Deno.test("an occurrence of another kind on a subscribed event's numbers is not delivered", async () => {
  const h = await attached();
  await subscribed(h, EVENT);
  const other = occurrence({ kind: Kind.Signal, iface: 1, ord: 3 }, 1n, [1]);
  h.socket.deliver(other);
  assert.deepEqual(h.occurrences, []);
  assert.deepEqual(h.notes, [
    { note: "discarded", reason: "not-subscribed", frame: other },
  ]);
});

Deno.test("connect rejects, and does not throw, when the platform refuses the URL", async () => {
  let pending: Promise<Session> | undefined;
  assert.doesNotThrow(() => {
    pending = connect("not a url", { catalog: { name: "v", hash: HASH } });
  });
  await assert.rejects(
    pending!,
    (e: unknown) => e instanceof Error && e.name === "SyntaxError",
  );
});

Deno.test("a refused attach closes with 1000 when the server does not close, and closed resolves on the close event", async () => {
  const h = start();
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  socket.deliver({
    form: "attached",
    outcome: { outcome: "refused", reason: "encoding-unsupported" },
  });
  let refusal: AttachRefused | undefined;
  await h.pending.catch((e) => {
    refusal = e;
  });
  assert.ok(refusal instanceof AttachRefused);
  // The client closes the connection itself.
  assert.deepEqual(socket.closes, [{ code: 1000 }]);
  // The closing handshake the client started completes.
  socket.serverClose(1000);
  assert.deepEqual(await refusal.closed, {
    code: 1000,
    reason: "",
    refused: "encoding-unsupported",
  });
});

Deno.test("a refused attach ends the session: a later message records nothing", async () => {
  const h = start();
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  socket.deliver({
    form: "attached",
    outcome: { outcome: "refused", reason: "catalog-mismatch" },
  });
  await assert.rejects(h.pending, AttachRefused);
  socket.deliver("hello");
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  assert.deepEqual(h.notes, []);
  assert.deepEqual(socket.closes, [{ code: 1000 }]);
  socket.serverClose(1000);
});

Deno.test("a read of a subscribed signal settled by a live publish ignores its own reply with the same seq", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  const reading = h.session.read(SIGNAL);
  h.socket.deliver(live(SIGNAL, 6n, [6]));
  // The read's own reply: the server reuses the seq of the identical sample.
  h.socket.deliver(live(SIGNAL, 6n, [6]));
  const sample = await reading;
  assert.equal(sample.envelope.seq, 6n);
  assert.equal(h.session.sample(1, 2)?.envelope.seq, 6n);
  assert.equal(h.publishes.length, 1);
  assert.deepEqual(h.notes, []);
});

Deno.test("two different operations waiting on one identity are answered in order", async () => {
  const h = await attached();
  const subscribing = h.session.subscribe(SIGNAL);
  const reading = h.session.read(SIGNAL);
  h.socket.deliver(accepted(SIGNAL));
  h.socket.deliver(refusedAnswer(SIGNAL));
  assert.deepEqual(h.notes, []);
  await subscribing;
  await assert.rejects(reading, unknownInteraction);
});

Deno.test("a frame that arrives after close records nothing and leaves the violation unset", async () => {
  const h = await attached();
  h.session.close();
  h.socket.deliver("hello");
  h.socket.deliver(live(SIGNAL, 1n, [1]));
  assert.deepEqual(h.notes, []);
  assert.deepEqual(h.socket.closes, [{ code: 1000 }]);
  h.socket.serverClose(1000);
  assert.deepEqual(await h.session.closed, { code: 1000, reason: "" });
});

Deno.test("the default request stamp is the wall clock in microseconds", async () => {
  const h = await attached();
  const before = BigInt(Date.now()) * 1000n;
  const calling = h.session.command(1, 9, new Uint8Array(0), { bound: 60_000 });
  const after = BigInt(Date.now()) * 1000n;
  const sent = lastSent(h.socket);
  assert.equal(sent.form, "request");
  const stamp = sent.form === "request" ? sent.envelope.stamp : -1n;
  assert.ok(
    before <= stamp && stamp <= after,
    `stamp ${stamp} is not between ${before} and ${after}`,
  );
  h.session.close();
  await assert.rejects(calling, isFailure("down"));
});

Deno.test("a discard as the first message, before attached, rejects connect with unexpected-form", async () => {
  const h = start();
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  // An `Init` publish, which carries no payload, with one byte too many: the
  // codec discards it.
  const init = encode(publish(SIGNAL, 1n, { provenance: "init" }));
  const message = new Uint8Array(init.length + 1);
  message.set(init);
  socket.deliver(message);
  await assert.rejects(
    h.pending,
    (e: unknown) => e instanceof ProtocolError && e.cause === "unexpected-form",
  );
  assert.deepEqual(socket.closes, [{}]);
});

Deno.test("unsubscribe after the session ended sends nothing", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  h.socket.serverClose(1001);
  const sent = h.socket.sent.length;
  assert.doesNotThrow(() => h.session.unsubscribe(SIGNAL));
  assert.equal(h.socket.sent.length, sent);
});

Deno.test("a second close, or a close after a violation, sends no second close", async () => {
  const h = await attached();
  h.session.close();
  h.session.close();
  assert.deepEqual(h.socket.closes, [{ code: 1000 }]);

  const v = await attached();
  v.socket.deliver("hello");
  v.session.close();
  assert.deepEqual(v.socket.closes, [{}]);
});

Deno.test("the waiting queue is per kind: a read of a fixed and a subscribe of a signal with the same numbers are answered separately", async () => {
  const h = await attached();
  const fixed: Target = { kind: Kind.Fixed, iface: 1, ord: 5 };
  const signal: Target = { kind: Kind.Signal, iface: 1, ord: 5 };
  const reading = h.session.read(fixed);
  const subscribing = h.session.subscribe(signal);
  h.socket.deliver(accepted(signal));
  h.socket.deliver(live(fixed, 0n, [0x0f]));
  assert.deepEqual(h.notes, []);
  await subscribing;
  assert.deepEqual((await reading).payload, Uint8Array.from([0x0f]));
  assert.equal(h.session.sample(1, 5), undefined);
});

Deno.test("the end of the session clears the held samples", async () => {
  const h = await attached();
  await subscribed(h, SIGNAL);
  h.socket.deliver(live(SIGNAL, 1n, [1]));
  assert.equal(h.session.sample(1, 2)?.envelope.seq, 1n);
  h.socket.serverClose(1001);
  assert.equal(h.session.sample(1, 2), undefined);
});

Deno.test("the encoding option sets the attach's encoding tag", () => {
  for (const [encoding, tag] of [["flatbuffers", 1], ["repr-c", 3]] as const) {
    const h = start("veh.cabin", { encoding });
    h.sockets[0].open(SUBPROTOCOL);
    const sent = lastSent(h.sockets[0]);
    assert.equal(sent.form === "attach" && sent.encoding, tag, encoding);
    h.sockets[0].serverClose(1000);
    h.pending.catch(() => {});
  }
});

Deno.test("an interface number or an ordinal outside 1 to 4294967295 is a RangeError and sends nothing", async () => {
  const h = await attached();
  const sent = h.socket.sent.length;
  const bad: Target[] = [
    { kind: Kind.Signal, iface: 0, ord: 2 },
    { kind: Kind.Signal, iface: 1, ord: 2.5 },
    { kind: Kind.Signal, iface: -1, ord: 2 },
    { kind: Kind.Signal, iface: 1, ord: 4_294_967_296 },
    { kind: Kind.Signal, iface: Number.NaN, ord: 2 },
  ];
  for (const t of bad) {
    await assert.rejects(h.session.subscribe(t), RangeError);
    await assert.rejects(h.session.read(t), RangeError);
    assert.throws(() => h.session.unsubscribe(t), RangeError);
    await assert.rejects(
      h.session.command(t.iface, t.ord, new Uint8Array(0), { bound: 1000 }),
      RangeError,
    );
    await assert.rejects(
      h.session.query(t.iface, t.ord, new Uint8Array(0), { bound: 1000 }),
      RangeError,
    );
  }
  assert.equal(h.socket.sent.length, sent);
  // The largest numbers are accepted.
  const reading = h.session.read({
    kind: Kind.Fixed,
    iface: 4_294_967_295,
    ord: 4_294_967_295,
  });
  assert.equal(h.socket.sent.length, sent + 1);
  h.session.close();
  await assert.rejects(reading, isFailure("down"));
});
