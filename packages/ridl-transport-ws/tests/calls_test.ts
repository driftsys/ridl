import assert from "node:assert/strict";
import { connect, decode, Kind, SessionError, SUBPROTOCOL } from "../mod.ts";
import type { Frame, Note, ResponseOutcome, Session, Target } from "../mod.ts";
import { FakeSocket } from "./fake_socket.ts";

const HASH = Uint8Array.from({ length: 32 }, (_, i) => i);
const STAMP = 0x0123456789ABn;
const COMMAND: Target = { kind: Kind.Command, iface: 1, ord: 6 };
const QUERY: Target = { kind: Kind.Query, iface: 1, ord: 7 };
const SIGNAL: Target = { kind: Kind.Signal, iface: 1, ord: 2 };
const FIXED: Target = { kind: Kind.Fixed, iface: 1, ord: 5 };
const BOUND = { bound: 20 };

// Deno 2 runs a test with the op sanitizer off unless the test turns it on.
// Every test here turns it on, so a call's timer left running when a test ends
// fails that test.
function test(name: string, fn: () => Promise<void>): void {
  Deno.test({ name, sanitizeOps: true, fn });
}

async function attached() {
  const sockets: FakeSocket[] = [];
  const notes: Note[] = [];
  const pending = connect("ws://127.0.0.1:9000", {
    catalog: { name: "veh.cabin", hash: HASH },
    clock: () => STAMP,
    onNote: (n) => notes.push(n),
    webSocket: (url, protocols) => {
      const socket = new FakeSocket(url, protocols);
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
  });
  const socket = sockets[0];
  socket.open(SUBPROTOCOL);
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

function lastSent(socket: FakeSocket): Frame {
  const all = frames(socket);
  return all[all.length - 1];
}

function response(
  target: Target,
  correlation: bigint,
  outcome: ResponseOutcome,
): Frame {
  return {
    form: "response",
    target,
    envelope: { stamp: 500n, seq: 1n },
    correlation,
    outcome,
  };
}

function isFailure(category: string) {
  return (e: unknown) =>
    e instanceof SessionError && e.failure.category === category;
}

test("a command sends a request with seq 1 and the caller stamp, and resolves on accepted", async () => {
  const { socket, session, notes } = await attached();
  const args = Uint8Array.from([1, 2, 3]);
  const call = session.command(1, 6, args, BOUND);
  assert.deepEqual(lastSent(socket), {
    form: "request",
    target: COMMAND,
    envelope: { stamp: STAMP, seq: 1n },
    payload: Uint8Array.from([1, 2, 3]),
  });
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  assert.equal(await call, undefined);
  assert.deepEqual(notes, []);
});

test("a second call takes seq 2", async () => {
  const { socket, session } = await attached();
  const first = session.command(1, 6, new Uint8Array(), BOUND);
  const second = session.query(1, 7, new Uint8Array(), BOUND);
  const sent = frames(socket).slice(-2);
  assert.equal(sent[0].form === "request" && sent[0].envelope.seq, 1n);
  assert.equal(sent[1].form === "request" && sent[1].envelope.seq, 2n);
  assert.equal(sent[1].form === "request" && sent[1].target.kind, Kind.Query);
  socket.deliver(
    response(QUERY, 2n, { outcome: "reply", payload: Uint8Array.of(9) }),
  );
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  assert.deepEqual(await second, Uint8Array.of(9));
  await first;
});

test("the payload is copied into the request", async () => {
  const { socket, session } = await attached();
  const args = Uint8Array.from([4, 5]);
  const call = session.command(1, 6, args, BOUND);
  args[0] = 99;
  const sent = lastSent(socket);
  assert.equal(sent.form, "request");
  if (sent.form === "request") {
    assert.deepEqual(sent.payload, Uint8Array.from([4, 5]));
  }
  assert.deepEqual(args, Uint8Array.from([99, 5]));
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  await call;
});

test("a query resolves with the reply payload", async () => {
  const { socket, session } = await attached();
  const call = session.query(1, 7, Uint8Array.of(1), BOUND);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: Uint8Array.of(7, 8, 9) }),
  );
  const reply = await call;
  assert.ok(reply instanceof Uint8Array);
  assert.deepEqual(reply, Uint8Array.of(7, 8, 9));
});

test("a zero-length reply resolves with an empty Uint8Array", async () => {
  const { socket, session } = await attached();
  const call = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: new Uint8Array() }),
  );
  const reply = await call;
  assert.ok(reply instanceof Uint8Array);
  assert.equal(reply.length, 0);
});

test("contract invalid-value with rule range rejects with that contract", async () => {
  const { socket, session } = await attached();
  const call = session.command(1, 6, new Uint8Array(), BOUND);
  socket.deliver(
    response(COMMAND, 1n, {
      outcome: "contract",
      contract: { category: "invalid-value", rule: "range" },
    }),
  );
  await assert.rejects(call, (e: unknown) => {
    assert.ok(e instanceof SessionError);
    assert.deepEqual(e.failure, {
      category: "contract",
      contract: { category: "invalid-value", rule: "range" },
    });
    return true;
  });
});

test("corrupt and busy reject with their categories", async () => {
  const { socket, session, notes } = await attached();
  const corrupt = session.command(1, 6, new Uint8Array(), BOUND);
  const busy = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(response(COMMAND, 1n, { outcome: "corrupt" }));
  socket.deliver(response(QUERY, 2n, { outcome: "busy" }));
  await assert.rejects(corrupt, isFailure("corrupt"));
  await assert.rejects(busy, isFailure("busy"));
  // Each call is forgotten once settled: a second response is unknown.
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
    "unknown-correlation",
  ]);
});

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

test("a command whose response arrives before its bound resolves", async () => {
  const { socket, session } = await attached();
  const call = session.command(1, 6, new Uint8Array(), { bound: 500 });
  await sleep(10);
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  assert.equal(await call, undefined);
});

test("a query waits for its bound before it rejects timeout", async () => {
  const { session } = await attached();
  let settled = false;
  const call = session.query(1, 7, new Uint8Array(), { bound: 200 });
  const watched = call.then(
    () => settled = true,
    () => settled = true,
  );
  await sleep(20);
  assert.equal(settled, false);
  await assert.rejects(call, isFailure("timeout"));
  await watched;
});

test("a contract response that echoes another kind settles the call", async () => {
  const { socket, session, notes } = await attached();
  const call = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(
    response(COMMAND, 1n, {
      outcome: "contract",
      contract: { category: "unknown-interaction" },
    }),
  );
  await assert.rejects(call, (e: unknown) =>
    e instanceof SessionError &&
    e.failure.category === "contract" &&
    e.failure.contract.category === "unknown-interaction");
  assert.deepEqual(notes, []);
});

test("a command with no response within its bound rejects undelivered", async () => {
  const { session } = await attached();
  await assert.rejects(
    session.command(1, 6, new Uint8Array(), { bound: 5 }),
    isFailure("undelivered"),
  );
});

test("a query with no response within its bound rejects timeout", async () => {
  const { session } = await attached();
  await assert.rejects(
    session.query(1, 7, new Uint8Array(), { bound: 5 }),
    isFailure("timeout"),
  );
});

test("a response after the bound is discarded as unknown-correlation", async () => {
  const { socket, session, notes } = await attached();
  await assert.rejects(
    session.query(1, 7, new Uint8Array(), { bound: 5 }),
    isFailure("timeout"),
  );
  const late = response(QUERY, 1n, {
    outcome: "reply",
    payload: Uint8Array.of(1),
  });
  socket.deliver(late);
  assert.equal(notes.length, 1);
  const note = notes[0];
  assert.equal(note.note, "discarded");
  if (note.note === "discarded") {
    assert.equal(note.reason, "unknown-correlation");
    assert.deepEqual(note.frame, late);
  }
  assert.deepEqual(socket.closes, []);
});

test("a response with an unknown correlation is discarded and the session continues", async () => {
  const { socket, session, notes } = await attached();
  const call = session.command(1, 6, new Uint8Array(), BOUND);
  socket.deliver(response(COMMAND, 42n, { outcome: "accepted" }));
  assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
    "unknown-correlation",
  ]);
  assert.deepEqual(socket.closes, []);
  // The call in flight is untouched, and the session goes on.
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  await call;
  const read = session.read(FIXED);
  socket.deliver({
    form: "publish",
    target: FIXED,
    envelope: { stamp: 1n, seq: 1n },
    state: { provenance: "live", payload: Uint8Array.of(3) },
  });
  assert.deepEqual((await read).payload, Uint8Array.of(3));
  assert.equal(notes.length, 1);
});

test("the session ending rejects a call in flight, a waiting subscribe and a waiting read with down, and closed resolves once", async () => {
  const { socket, session } = await attached();
  const call = session.query(1, 7, new Uint8Array(), BOUND);
  const sub = session.subscribe(SIGNAL);
  const read = session.read(FIXED);
  let ends = 0;
  const closed = session.closed.then((ended) => {
    ends += 1;
    return ended;
  });
  socket.serverClose(1001, "going away");
  await assert.rejects(call, isFailure("down"));
  await assert.rejects(sub, isFailure("down"));
  await assert.rejects(read, isFailure("down"));
  assert.deepEqual(await closed, { code: 1001, reason: "going away" });
  // A second end report changes nothing.
  socket.error();
  await Promise.resolve();
  assert.equal(ends, 1);
  // The test ends before the call's bound passes, so Deno's sanitizer reports
  // the call's timer as leaked unless the end of the session cleared it.
});

test("a call after the session ended rejects down and sends nothing", async () => {
  const { socket, session } = await attached();
  session.close();
  const before = socket.sent.length;
  await assert.rejects(
    session.command(1, 6, new Uint8Array(), BOUND),
    isFailure("down"),
  );
  await assert.rejects(
    session.query(1, 7, new Uint8Array(), BOUND),
    isFailure("down"),
  );
  assert.equal(socket.sent.length, before);
  socket.serverClose(1000);
  await session.closed;
});

test("a bound of 0 or NaN rejects with RangeError and sends nothing", async () => {
  const { socket, session } = await attached();
  const before = socket.sent.length;
  for (const bound of [0, NaN, -1, Infinity, 2_147_483_648]) {
    await assert.rejects(
      session.command(1, 6, new Uint8Array(), { bound }),
      RangeError,
    );
    await assert.rejects(
      session.query(1, 7, new Uint8Array(), { bound }),
      RangeError,
    );
  }
  assert.equal(socket.sent.length, before);
  // A refused bound does not take a caller number.
  const call = session.command(1, 6, new Uint8Array(), BOUND);
  const sent = lastSent(socket);
  assert.equal(sent.form === "request" && sent.envelope.seq, 1n);
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  await call;
  // The longest delay `setTimeout` takes is a valid bound.
  const longest = session.query(1, 7, new Uint8Array(), {
    bound: 2_147_483_647,
  });
  socket.deliver(
    response(QUERY, 2n, { outcome: "reply", payload: Uint8Array.of(1) }),
  );
  assert.deepEqual(await longest, Uint8Array.of(1));
});

test("a response whose outcome does not fit the call's kind is discarded and the call stays in flight", async () => {
  const { socket, session, notes } = await attached();
  const query = session.query(1, 7, new Uint8Array(), BOUND);
  const command = session.command(1, 6, new Uint8Array(), BOUND);
  socket.deliver(response(QUERY, 1n, { outcome: "accepted" }));
  socket.deliver(
    response(COMMAND, 2n, { outcome: "reply", payload: Uint8Array.of(1) }),
  );
  assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
    "unexpected-outcome",
    "unexpected-outcome",
  ]);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: Uint8Array.of(2) }),
  );
  socket.deliver(response(COMMAND, 2n, { outcome: "accepted" }));
  assert.deepEqual(await query, Uint8Array.of(2));
  await command;
});

test("a reply longer than the query's default limit rejects with corrupt", async () => {
  const { socket, session, notes } = await attached();
  const call = session.query(1, 7, new Uint8Array(), BOUND);
  // A reply of 16,129 bytes: one byte above the default ceiling of a query.
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: new Uint8Array(16_129) }),
  );
  await assert.rejects(call, isFailure("corrupt"));
  assert.deepEqual(notes, []);
});

test("a reply of exactly the query's default limit resolves", async () => {
  const { socket, session } = await attached();
  const call = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: new Uint8Array(16_128) }),
  );
  assert.equal((await call).length, 16_128);
});
