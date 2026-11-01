import assert from "node:assert/strict";
import {
  connect,
  decode,
  Kind,
  SessionError,
  SUBPROTOCOL,
  SUBPROTOCOL_V3,
} from "../mod.ts";
import type {
  ConnectOptions,
  Frame,
  Note,
  ResponseOutcome,
  Session,
  StreamOutcome,
  Target,
} from "../mod.ts";
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

async function attached(
  extra: Partial<ConnectOptions> = {},
  protocol = SUBPROTOCOL,
) {
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
    ...extra,
  });
  const socket = sockets[0];
  socket.open(protocol);
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

test("a reply is checked against the query's limit, not against the limit of the kind its target echoes", async () => {
  const { socket, session } = await attached();
  const call = session.query(1, 7, new Uint8Array(), BOUND);
  // The response echoes the query's numbers under the fixed kind, whose
  // default ceiling (65,280) would let 20,000 bytes pass.
  const echoed: Target = { kind: Kind.Fixed, iface: 1, ord: 7 };
  socket.deliver(
    response(echoed, 1n, { outcome: "reply", payload: new Uint8Array(20_000) }),
  );
  await assert.rejects(call, isFailure("corrupt"));
});

/** The generated table of an interface whose query `7` is raised to 20,000 bytes. */
const RAISED_QUERY = {
  iface: 1,
  deep: { iface: 1, ord: 7, kind: "query", limit: 20_000 },
} as const;

test("a reply on a query the generated tables raise resolves up to the raised limit and rejects corrupt above it", async () => {
  const { socket, session } = await attached({ interfaces: [RAISED_QUERY] });
  const within = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: new Uint8Array(20_000) }),
  );
  await assert.doesNotReject(within);
  assert.equal((await within).length, 20_000);
  const above = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(
    response(QUERY, 2n, { outcome: "reply", payload: new Uint8Array(20_001) }),
  );
  await assert.rejects(above, isFailure("corrupt"));
});

/**
 * The generated table of an interface whose command `6` and query `7` are
 * lowered to 100 bytes, below the default ceiling of their kind.
 */
const LOWERED_CALLS = {
  iface: 1,
  set: { iface: 1, ord: 6, kind: "command", limit: 100 },
  ask: { iface: 1, ord: 7, kind: "query", limit: 100 },
} as const;

test("a reply on a query the generated tables lower resolves up to the lowered limit and rejects corrupt above it", async () => {
  const { socket, session } = await attached({ interfaces: [LOWERED_CALLS] });
  const within = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: new Uint8Array(100) }),
  );
  assert.equal((await within).length, 100);
  const above = session.query(1, 7, new Uint8Array(), BOUND);
  socket.deliver(
    response(QUERY, 2n, { outcome: "reply", payload: new Uint8Array(101) }),
  );
  await assert.rejects(above, isFailure("corrupt"));
});

test("a command whose arguments are above the default ceiling of a command rejects with corrupt and sends nothing", async () => {
  const { socket, session, notes } = await attached();
  const sent = frames(socket).length;
  await assert.rejects(
    session.command(1, 6, new Uint8Array(16_129), BOUND),
    isFailure("corrupt"),
  );
  assert.equal(frames(socket).length, sent);
  // The refused call took no caller number: the next request has `seq` 1,
  // and arguments of exactly the ceiling are sent.
  const call = session.command(1, 6, new Uint8Array(16_128), BOUND);
  const request = lastSent(socket);
  assert.equal(request.form, "request");
  if (request.form !== "request") return;
  assert.equal(request.envelope.seq, 1n);
  assert.equal(request.payload.length, 16_128);
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  await call;
  assert.deepEqual(notes, []);
});

test("a query whose arguments are above the default ceiling of a query rejects with corrupt and sends nothing", async () => {
  const { socket, session } = await attached();
  const sent = frames(socket).length;
  await assert.rejects(
    session.query(1, 7, new Uint8Array(16_129), BOUND),
    isFailure("corrupt"),
  );
  assert.equal(frames(socket).length, sent);
});

test("the arguments of a call are checked against the limit the generated tables give the member", async () => {
  const { socket, session } = await attached({
    interfaces: [LOWERED_CALLS, RAISED_QUERY],
  });
  // The command is lowered to 100 bytes: 101 are refused, 100 are sent.
  const sent = frames(socket).length;
  await assert.rejects(
    session.command(1, 6, new Uint8Array(101), BOUND),
    isFailure("corrupt"),
  );
  assert.equal(frames(socket).length, sent);
  const within = session.command(1, 6, new Uint8Array(100), BOUND);
  assert.equal(frames(socket).length, sent + 1);
  socket.deliver(response(COMMAND, 1n, { outcome: "accepted" }));
  await within;
  // A member of another kind with the same numbers keeps its own limit: the
  // query 6 has no entry, so its arguments are checked against 16,128.
  const other = session.query(1, 6, new Uint8Array(16_128), BOUND);
  assert.equal(frames(socket).length, sent + 2);
  socket.deliver(
    response({ kind: Kind.Query, iface: 1, ord: 6 }, 2n, {
      outcome: "reply",
      payload: new Uint8Array(),
    }),
  );
  await other;
});

test("the arguments of a query the generated tables raise are sent up to the raised limit", async () => {
  const { socket, session } = await attached({ interfaces: [RAISED_QUERY] });
  const sent = frames(socket).length;
  const within = session.query(1, 7, new Uint8Array(20_000), BOUND);
  assert.equal(frames(socket).length, sent + 1);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: new Uint8Array() }),
  );
  await within;
  await assert.rejects(
    session.query(1, 7, new Uint8Array(20_001), BOUND),
    isFailure("corrupt"),
  );
  assert.equal(frames(socket).length, sent + 1);
});

// A query whose reply is a stream, on a frame version 3 session.

const STREAMED: Target = { kind: Kind.Query, iface: 1, ord: 10 };

function attachedV3(extra: Partial<ConnectOptions> = {}) {
  return attached(extra, SUBPROTOCOL_V3);
}

function chunk(
  correlation: bigint,
  seq: bigint,
  payload: number[],
  stream = 0,
  target = STREAMED,
): Frame {
  return {
    form: "chunk",
    target,
    envelope: { stamp: 600n + seq, seq },
    correlation,
    stream,
    payload: Uint8Array.from(payload),
  };
}

function streamEnd(
  correlation: bigint,
  seq: bigint,
  outcome: StreamOutcome,
  target = STREAMED,
): Frame {
  return {
    form: "stream_end",
    target,
    envelope: { stamp: 600n + seq, seq },
    correlation,
    stream: 0,
    outcome,
  };
}

/** The `stream_end` the client sends to stop the reply stream of call 1. */
function stopped(outcome: StreamOutcome): Frame {
  return {
    form: "stream_end",
    target: STREAMED,
    envelope: { stamp: STAMP, seq: 1n },
    correlation: 1n,
    stream: 0,
    outcome,
  };
}

function sentOf(socket: FakeSocket, form: Frame["form"]): Frame[] {
  return frames(socket).filter((f) => f.form === form);
}

/** The sum of the `stream_credit` frames the client sent. */
function credited(socket: FakeSocket): number {
  return sentOf(socket, "stream_credit").reduce(
    (sum, f) => sum + (f.form === "stream_credit" ? f.credit : 0),
    0,
  );
}

/** Lets the stream run the callbacks it queued. */
function settled(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

test("queryStream_reads_chunks_in_order_and_closes_on_end", async () => {
  const { socket, session, notes } = await attachedV3();
  const stream = session.queryStream(1, 10, Uint8Array.of(3), BOUND);
  assert.deepEqual(lastSent(socket), {
    form: "request",
    target: STREAMED,
    envelope: { stamp: STAMP, seq: 1n },
    payload: Uint8Array.of(3),
  });
  socket.deliver(chunk(1n, 1n, [1, 1]));
  socket.deliver(chunk(1n, 2n, [2]));
  socket.deliver(chunk(1n, 3n, []));
  socket.deliver(streamEnd(1n, 4n, { kind: "end" }));
  const read: Uint8Array[] = [];
  for await (const c of stream) read.push(c);
  assert.deepEqual(read, [
    Uint8Array.of(1, 1),
    Uint8Array.of(2),
    new Uint8Array(),
  ]);
  assert.deepEqual(notes, []);
  assert.deepEqual(sentOf(socket, "stream_end"), []);
});

test("queryStream_grants_credit_as_the_reader_drains", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(9), BOUND)
    .getReader();
  await settled();
  assert.equal(sentOf(socket, "stream_credit").length, 0);
  for (let seq = 1n; seq <= 4n; seq++) {
    socket.deliver(chunk(1n, seq, [Number(seq)]));
  }
  await settled();
  // The queue holds the four chunks of the initial credit: nothing is granted.
  assert.equal(sentOf(socket, "stream_credit").length, 0);
  for (let reads = 1; reads <= 8; reads++) {
    const { value, done } = await reader.read();
    assert.equal(done, false);
    assert.deepEqual(value, Uint8Array.of(reads));
    await settled();
    assert.equal(sentOf(socket, "stream_credit").length, reads);
    assert.ok(
      4 + credited(socket) <= 4 + reads,
      `granted ${4 + credited(socket)} after ${reads} reads`,
    );
    for (const f of sentOf(socket, "stream_credit")) {
      assert.deepEqual(f, {
        form: "stream_credit",
        target: STREAMED,
        correlation: 1n,
        stream: 0,
        credit: 1,
      });
    }
    socket.deliver(chunk(1n, BigInt(reads + 4), [reads + 4]));
  }
  socket.deliver(streamEnd(1n, 13n, { kind: "end" }));
  for (let n = 9; n <= 12; n++) {
    assert.deepEqual((await reader.read()).value, Uint8Array.of(n));
  }
  assert.equal((await reader.read()).done, true);
  assert.deepEqual(notes, []);
});

test("queryStream_seq_gap_errors_interrupted_and_sends_stream_end", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(3), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1]));
  socket.deliver(chunk(1n, 3n, [3]));
  await assert.rejects(reader.read(), isFailure("interrupted"));
  assert.deepEqual(sentOf(socket, "stream_end"), [
    stopped({ kind: "interrupted" }),
  ]);
  // The call is forgotten: a later frame of it is discarded.
  socket.deliver(chunk(1n, 4n, [4]));
  assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
    "unknown-correlation",
  ]);
  assert.deepEqual(socket.closes, []);
});

test("a stream_end after a gap errors interrupted", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(3), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1]));
  socket.deliver(streamEnd(1n, 3n, { kind: "end" }));
  await assert.rejects(reader.read(), isFailure("interrupted"));
  // The stream has ended: the client has nothing to stop.
  assert.deepEqual(sentOf(socket, "stream_end"), []);
  assert.deepEqual(notes, []);
});

test("queryStream_chunk_beyond_credit_is_a_protocol_error", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(9), BOUND)
    .getReader();
  for (let seq = 1n; seq <= 4n; seq++) socket.deliver(chunk(1n, seq, [1]));
  assert.deepEqual(notes, []);
  socket.deliver(chunk(1n, 5n, [1]));
  assert.deepEqual(notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
  assert.deepEqual(socket.closes, [{}]);
  await assert.rejects(reader.read(), isFailure("down"));
});

test("queryStream_response_is_a_protocol_error", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  socket.deliver(
    response(STREAMED, 1n, { outcome: "reply", payload: Uint8Array.of(1) }),
  );
  assert.deepEqual(notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
  assert.deepEqual(socket.closes, [{}]);
  await assert.rejects(reader.read(), isFailure("down"));
});

test("a_refusal_response_errors_the_stream_and_keeps_the_session", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  socket.deliver(
    response(STREAMED, 1n, {
      outcome: "contract",
      contract: { category: "invalid-value", rule: "range" },
    }),
  );
  await assert.rejects(reader.read(), (e: unknown) => {
    assert.ok(e instanceof SessionError);
    assert.deepEqual(e.failure, {
      category: "contract",
      contract: { category: "invalid-value", rule: "range" },
    });
    return true;
  });
  // The call is forgotten, and the stream ended at the server: nothing is
  // sent back, and a later frame of the call is discarded.
  assert.deepEqual(sentOf(socket, "stream_end"), []);
  socket.deliver(streamEnd(1n, 1n, { kind: "end" }));
  assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
    "unknown-correlation",
  ]);
  assert.deepEqual(socket.closes, []);
  // The session goes on.
  const call = session.query(1, 7, Uint8Array.of(1), BOUND);
  socket.deliver(
    response(QUERY, 2n, { outcome: "reply", payload: Uint8Array.of(2) }),
  );
  assert.deepEqual(await call, Uint8Array.of(2));
});

test("a corrupt or busy response errors the stream with that category", async () => {
  for (const outcome of ["corrupt", "busy"] as const) {
    const { socket, session, notes } = await attachedV3();
    const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
      .getReader();
    socket.deliver(response(STREAMED, 1n, { outcome }));
    await assert.rejects(reader.read(), isFailure(outcome));
    assert.deepEqual(notes, []);
    assert.deepEqual(socket.closes, []);
  }
});

test("an accepted response for a streamed query is a protocol error", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  socket.deliver(response(STREAMED, 1n, { outcome: "accepted" }));
  assert.deepEqual(notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
  await assert.rejects(reader.read(), isFailure("down"));
});

test("a refusing response after a chunk is a protocol error", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(2), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1]));
  socket.deliver(response(STREAMED, 1n, { outcome: "busy" }));
  assert.deepEqual(notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
  assert.deepEqual(socket.closes, [{}]);
  await assert.rejects(reader.read(), isFailure("down"));
});

test("a stream_end forgets the call: a later chunk of it is discarded", async () => {
  for (const outcome of [{ kind: "end" }, { kind: "busy" }] as const) {
    const { socket, session, notes } = await attachedV3();
    const stream = session.queryStream(1, 10, Uint8Array.of(1), BOUND);
    const read = Array.fromAsync(stream).catch((e) => e);
    if (outcome.kind === "end") socket.deliver(chunk(1n, 1n, [1]));
    const seq = outcome.kind === "end" ? 2n : 1n;
    socket.deliver(streamEnd(1n, seq, outcome));
    socket.deliver(chunk(1n, seq + 1n, [9]));
    assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
      "unknown-correlation",
    ]);
    assert.deepEqual(socket.closes, []);
    const result = await read;
    if (outcome.kind === "end") assert.deepEqual(result, [Uint8Array.of(1)]);
    else assert.ok(isFailure("busy")(result));
  }
});

test("a repeated chunk seq errors interrupted and sends stream_end", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(3), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1]));
  socket.deliver(chunk(1n, 1n, [1]));
  await assert.rejects(reader.read(), isFailure("interrupted"));
  assert.deepEqual(sentOf(socket, "stream_end"), [
    stopped({ kind: "interrupted" }),
  ]);
  assert.deepEqual(notes, []);
});

test("a_chunk_for_a_plain_query_is_a_protocol_error", async () => {
  const { socket, session, notes } = await attachedV3();
  const call = session.query(1, 7, Uint8Array.of(1), BOUND);
  const down = assert.rejects(call, isFailure("down"));
  socket.deliver(chunk(1n, 1n, [1], 0, QUERY));
  assert.deepEqual(notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
  assert.deepEqual(socket.closes, [{}]);
  await down;
});

test("a stream_end or a stream_credit for a plain query is a protocol error", async () => {
  const frames: Frame[] = [
    streamEnd(1n, 1n, { kind: "end" }, QUERY),
    {
      form: "stream_credit",
      target: QUERY,
      correlation: 1n,
      stream: 0,
      credit: 1,
    },
  ];
  for (const frame of frames) {
    const { socket, session, notes } = await attachedV3();
    const down = assert.rejects(
      session.query(1, 7, Uint8Array.of(1), BOUND),
      isFailure("down"),
    );
    socket.deliver(frame);
    assert.deepEqual(notes, [{
      note: "violation",
      violation: "unexpected-form",
    }]);
    await down;
  }
});

test("a stream_credit, or a chunk on a stream other than 0, for a streamed reply is a protocol error", async () => {
  const frames: Frame[] = [
    {
      form: "stream_credit",
      target: STREAMED,
      correlation: 1n,
      stream: 0,
      credit: 1,
    },
    chunk(1n, 1n, [1], 1),
  ];
  for (const frame of frames) {
    const { socket, session, notes } = await attachedV3();
    const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
      .getReader();
    socket.deliver(frame);
    assert.deepEqual(notes, [{
      note: "violation",
      violation: "unexpected-form",
    }]);
    await assert.rejects(reader.read(), isFailure("down"));
  }
});

test("cancel_sends_one_stream_end_interrupted_with_seq_1", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(9), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1]));
  socket.deliver(chunk(1n, 2n, [2]));
  await reader.cancel();
  await reader.cancel();
  assert.deepEqual(sentOf(socket, "stream_end"), [
    stopped({ kind: "interrupted" }),
  ]);
  assert.deepEqual(notes, []);
});

test("a_frame_after_cancel_is_discarded", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(9), BOUND)
    .getReader();
  await reader.cancel();
  socket.deliver(chunk(1n, 1n, [1]));
  socket.deliver(streamEnd(1n, 2n, { kind: "end" }));
  await settled();
  assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
    "unknown-correlation",
    "unknown-correlation",
  ]);
  assert.equal(sentOf(socket, "stream_credit").length, 0);
  assert.deepEqual(socket.closes, []);
});

test("an_empty_stream_closes_with_no_chunk", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(0), BOUND)
    .getReader();
  socket.deliver(streamEnd(1n, 1n, { kind: "end" }));
  assert.deepEqual(await reader.read(), { value: undefined, done: true });
  assert.deepEqual(notes, []);
});

test("queryStream_times_out_without_a_first_frame", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  await assert.rejects(reader.read(), isFailure("timeout"));
  assert.deepEqual(sentOf(socket, "stream_end"), [
    stopped({ kind: "interrupted" }),
  ]);
  socket.deliver(chunk(1n, 1n, [1]));
  assert.deepEqual(notes.map((n) => n.note === "discarded" && n.reason), [
    "unknown-correlation",
  ]);
});

test("a stream whose first chunk arrived does not time out", async () => {
  const { socket, session } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(2), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1]));
  await new Promise((resolve) => setTimeout(resolve, BOUND.bound * 3));
  socket.deliver(chunk(1n, 2n, [2]));
  socket.deliver(streamEnd(1n, 3n, { kind: "end" }));
  assert.deepEqual((await reader.read()).value, Uint8Array.of(1));
  assert.deepEqual((await reader.read()).value, Uint8Array.of(2));
  assert.equal((await reader.read()).done, true);
  assert.deepEqual(sentOf(socket, "stream_end"), []);
});

test("queryStream_on_a_v2_session_errors_unsupported_and_sends_nothing", async () => {
  const { socket, session, notes } = await attached();
  const sent = socket.sent.length;
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  await assert.rejects(reader.read(), isFailure("unsupported"));
  assert.equal(socket.sent.length, sent);
  // The session goes on, and the caller number was not used.
  const call = session.query(1, 7, new Uint8Array(), BOUND);
  const request = lastSent(socket);
  assert.equal(request.form === "request" && request.envelope.seq, 1n);
  socket.deliver(
    response(QUERY, 1n, { outcome: "reply", payload: new Uint8Array() }),
  );
  await call;
  assert.deepEqual(notes, []);
  assert.deepEqual(socket.closes, []);
});

test("stream_end_busy_before_a_chunk_errors_busy", async () => {
  const { socket, session, notes } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  socket.deliver(streamEnd(1n, 1n, { kind: "busy" }));
  await assert.rejects(reader.read(), isFailure("busy"));
  assert.deepEqual(sentOf(socket, "stream_end"), []);
  assert.deepEqual(notes, []);
});

test("a stream_end with a contract, corrupt or interrupted outcome errors with that category", async () => {
  const cases: [StreamOutcome, unknown][] = [
    [
      { kind: "contract", contract: { category: "unknown-interaction" } },
      { category: "contract", contract: { category: "unknown-interaction" } },
    ],
    [
      {
        kind: "contract",
        contract: { category: "invalid-value", rule: "length" },
      },
      {
        category: "contract",
        contract: { category: "invalid-value", rule: "length" },
      },
    ],
    [{ kind: "corrupt" }, { category: "corrupt" }],
    [{ kind: "interrupted" }, { category: "interrupted" }],
  ];
  for (const [outcome, failure] of cases) {
    const { socket, session } = await attachedV3();
    const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
      .getReader();
    socket.deliver(chunk(1n, 1n, [1]));
    socket.deliver(streamEnd(1n, 2n, outcome));
    await assert.rejects(reader.read(), (e: unknown) => {
      assert.ok(e instanceof SessionError);
      assert.deepEqual(e.failure, failure);
      return true;
    });
  }
});

const SMALL_STREAM = {
  iface: 1,
  ask: { iface: 1, ord: 10, kind: "query", limit: 2 },
} as const;

test("a chunk above the member's limit sends stream_end corrupt and errors corrupt", async () => {
  const { socket, session, notes } = await attachedV3({
    interfaces: [SMALL_STREAM],
  });
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1, 2]));
  assert.deepEqual(sentOf(socket, "stream_end"), []);
  socket.deliver(chunk(1n, 2n, [1, 2, 3]));
  assert.deepEqual(sentOf(socket, "stream_end"), [
    stopped({ kind: "corrupt" }),
  ]);
  await assert.rejects(reader.read(), isFailure("corrupt"));
  assert.deepEqual(notes, []);
});

test("the session ending errors a stream in flight with down", async () => {
  const { socket, session } = await attachedV3();
  const reader = session.queryStream(1, 10, Uint8Array.of(1), BOUND)
    .getReader();
  socket.deliver(chunk(1n, 1n, [1]));
  socket.serverClose(1001);
  await assert.rejects(reader.read(), isFailure("down"));
  // After the end, a new stream errors down and sends nothing.
  const sent = socket.sent.length;
  const late = session.queryStream(1, 10, Uint8Array.of(1), BOUND).getReader();
  await assert.rejects(late.read(), isFailure("down"));
  assert.equal(socket.sent.length, sent);
});

test("queryStream with a bound of 0 errors with RangeError and sends nothing", async () => {
  const { socket, session } = await attachedV3();
  const sent = socket.sent.length;
  const reader = session.queryStream(1, 10, Uint8Array.of(1), { bound: 0 })
    .getReader();
  await assert.rejects(reader.read(), RangeError);
  assert.equal(socket.sent.length, sent);
});
