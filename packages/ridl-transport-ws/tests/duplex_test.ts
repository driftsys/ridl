import assert from "node:assert/strict";
import {
  connect,
  decode,
  Kind,
  SessionError,
  SUBPROTOCOL,
  SUBPROTOCOL_V1,
  SUBPROTOCOL_V3,
} from "../mod.ts";
import type { ConnectOptions, Frame, Note, Session, Target } from "../mod.ts";
import { FakeSocket } from "./fake_socket.ts";

const TARGET: Target = { kind: Kind.Query, iface: 1, ord: 14 };
const bytes = Uint8Array.of(7);
const tick = () => new Promise<void>((resolve) => setTimeout(resolve, 0));

function sent(socket: FakeSocket): Frame[] {
  return socket.sent.map((data) => {
    const result = decode(data);
    assert.ok(result.ok);
    return result.frame;
  });
}

async function rig(
  protocol = SUBPROTOCOL_V3,
  extra: Partial<ConnectOptions> = {},
) {
  let socket!: FakeSocket;
  const notes: Note[] = [];
  const pending = connect("ws://test", {
    catalog: { name: "test.ws", hash: new Uint8Array(32).fill(7) },
    onNote: (note) => notes.push(note),
    webSocket: (url, protocols) => {
      socket = new FakeSocket(url, protocols);
      return socket as unknown as WebSocket;
    },
    ...extra,
  });
  socket.open(protocol);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  const session = await pending;
  return { socket, session, notes };
}

function open(session: Session, socket: FakeSocket, bound = 1000) {
  const { upload, stream } = session.queryDuplex(1, 14, bytes, { bound });
  const writer = upload.getWriter();
  const reader = stream.getReader();
  writer.closed.catch(() => {});
  reader.closed.catch(() => {});
  assert.equal(sent(socket).filter((f) => f.form === "request").length, 1);
  return { writer, reader };
}

function chunk(socket: FakeSocket, seq = 1n, payload = bytes, target = TARGET) {
  socket.deliver({
    form: "chunk",
    target,
    correlation: 1n,
    stream: 0,
    envelope: { stamp: 0n, seq },
    payload,
  });
}

function end(
  socket: FakeSocket,
  stream = 0,
  seq = 1n,
  kind: "end" | "busy" | "interrupted" = "end",
  target = TARGET,
) {
  socket.deliver({
    form: "stream_end",
    target,
    correlation: 1n,
    stream,
    envelope: { stamp: 0n, seq },
    outcome: { kind },
  });
}

function credit(socket: FakeSocket, target = TARGET) {
  socket.deliver({
    form: "stream_credit",
    target,
    correlation: 1n,
    stream: 1,
    credit: 1,
  });
}

function observe<T>(promise: Promise<T>) {
  const result: { value?: T; error?: unknown; settled: boolean } = {
    settled: false,
  };
  promise.then((value) => {
    result.value = value;
    result.settled = true;
  }, (error) => {
    result.error = error;
    result.settled = true;
  });
  return result;
}

function category(error: unknown, expected: string) {
  assert.ok(error instanceof SessionError);
  assert.equal(error.failure.category, expected);
}

async function blocked(writer: WritableStreamDefaultWriter<Uint8Array>) {
  for (let i = 0; i < 4; i++) await writer.write(bytes);
  const fifth = observe(writer.write(bytes));
  await tick();
  assert.equal(fifth.settled, false, "the fifth write must wait for credit");
  return fifth;
}

Deno.test("duplex_credit_and_close", async () => {
  const { session, socket } = await rig();
  try {
    const { writer, reader } = open(session, socket);
    const fifth = await blocked(writer);
    chunk(socket);
    assert.deepEqual(await reader.read(), { done: false, value: bytes });
    await tick();
    assert.ok(
      sent(socket).some((f) =>
        f.form === "stream_credit" && f.stream === 0 && f.credit === 1
      ),
    );
    credit(socket);
    await tick();
    assert.equal(fifth.settled, true);
    assert.equal(fifth.error, undefined);
    assert.deepEqual(
      sent(socket).filter((f) => f.form === "chunk").map((
        f,
      ) => [f.stream, f.envelope.seq]),
      [[1, 1n], [1, 2n], [1, 3n], [1, 4n], [1, 5n]],
    );
    await writer.close();
    assert.ok(
      sent(socket).some((f) =>
        f.form === "stream_end" && f.stream === 1 && f.envelope.seq === 6n &&
        f.outcome.kind === "end"
      ),
    );
    chunk(socket, 2n, Uint8Array.of(8));
    end(socket, 0, 3n);
    assert.deepEqual(await reader.read(), {
      done: false,
      value: Uint8Array.of(8),
    });
    assert.equal((await reader.read()).done, true);
  } finally {
    session.close();
  }
});

Deno.test("duplex_target_check", async () => {
  const { session, socket, notes } = await rig();
  try {
    const { writer, reader } = open(session, socket);
    const fifth = await blocked(writer);
    for (
      const target of [{ ...TARGET, iface: 2 }, { ...TARGET, ord: 15 }, {
        ...TARGET,
        kind: Kind.Command,
      }]
    ) {
      credit(socket, target);
      chunk(socket, 1n, bytes, target);
      end(socket, 1, 1n, "busy", target);
    }
    await tick();
    assert.equal(fifth.settled, false);
    assert.equal(notes.filter((n) => n.note === "discarded").length, 9);
    chunk(socket);
    assert.deepEqual((await reader.read()).value, bytes);
    credit(socket);
    await tick();
    assert.equal(fifth.settled, true);
    await writer.close();
    end(socket, 0, 2n);
    assert.equal((await reader.read()).done, true);
  } finally {
    session.close();
  }
});

Deno.test("duplex_upload_abort_stops_both_streams", async () => {
  const { session, socket, notes } = await rig();
  try {
    const { writer, reader } = open(session, socket);
    await writer.write(bytes);
    const read = observe(reader.read());
    const abort = observe(writer.abort());
    await tick();
    assert.equal(abort.settled, true);
    const stops = sent(socket).filter((f) => f.form === "stream_end");
    assert.deepEqual(
      stops.map((f) => [f.stream, f.envelope.seq, f.outcome.kind]),
      [[1, 2n, "interrupted"], [0, 1n, "interrupted"]],
      "upload abort must stop the upload before the reply",
    );
    for (const stop of stops) {
      assert.deepEqual(stop.target, TARGET);
      assert.equal(stop.correlation, 1n);
    }
    assert.equal(read.settled, true);
    category(read.error, "interrupted");
    chunk(socket);
    assert.ok(
      notes.some((n) =>
        n.note === "discarded" && n.reason === "unknown-correlation"
      ),
    );
  } finally {
    session.close();
  }
});

Deno.test("duplex_cancel_and_abort", async () => {
  for (const action of ["cancel", "abort"] as const) {
    const { session, socket } = await rig();
    try {
      const { writer, reader } = open(session, socket);
      const fifth = await blocked(writer);
      const other = observe<unknown>(
        action === "cancel" ? writer.closed : reader.read(),
      );
      const stopping = observe(
        action === "cancel" ? reader.cancel() : writer.abort(),
      );
      await tick();
      assert.equal(
        stopping.settled,
        true,
        "interrupt must release a blocked write",
      );
      assert.equal(stopping.error, undefined, "interruption must resolve");
      assert.equal(fifth.settled, true);
      category(fifth.error, "interrupted");
      assert.equal(other.settled, true);
      category(other.error, "interrupted");
      const stops = sent(socket).filter((f) => f.form === "stream_end");
      assert.deepEqual(
        stops.map((f) => [f.stream, f.outcome.kind]),
        action === "cancel"
          ? [[0, "interrupted"]]
          : [[1, "interrupted"], [0, "interrupted"]],
      );
      credit(socket);
      await tick();
      assert.equal(sent(socket).filter((f) => f.form === "chunk").length, 4);
    } finally {
      session.close();
    }
  }
});

Deno.test("duplex_refusal_and_end", async () => {
  for (const raceCredit of [false, true]) {
    for (
      const source of ["response", "upload", "reply", "end", "down"] as const
    ) {
      const { session, socket } = await rig();
      try {
        const { writer, reader } = open(session, socket);
        const fifth = await blocked(writer);
        const read = observe(reader.read());
        // A terminal frame in the same turn must win over newly granted credit.
        if (raceCredit) credit(socket);
        if (source === "response") {
          socket.deliver({
            form: "response",
            target: TARGET,
            correlation: 1n,
            envelope: { seq: 1n, stamp: 0n },
            outcome: {
              outcome: "contract",
              contract: { category: "invalid-value", rule: "utf8" },
            },
          });
        }
        if (source === "upload") end(socket, 1, 1n, "busy");
        if (source === "reply") end(socket, 0, 1n, "busy");
        if (source === "end") end(socket);
        if (source === "down") session.close();
        await tick();
        assert.equal(fifth.settled, true);
        category(
          fifth.error,
          source === "end"
            ? "interrupted"
            : source === "down"
            ? "down"
            : source === "response"
            ? "contract"
            : "busy",
        );
        assert.equal(read.settled, true);
        if (source === "end") assert.equal(read.value?.done, true);
        else {assert.equal(
            read.error,
            fifth.error,
            "both directions retain the same error object",
          );}
        if (source === "response") {
          assert.deepEqual((read.error as SessionError).failure, {
            category: "contract",
            contract: { category: "invalid-value", rule: "utf8" },
          });
        }
      } finally {
        session.close();
      }
    }
  }
});

Deno.test("duplex_reply_validation", async () => {
  for (
    const defect of [
      "gap",
      "size",
      "accepted",
      "upload-chunk",
      "credit",
      "direction",
      "response",
      "late-response",
    ] as const
  ) {
    const { session, socket } = await rig();
    try {
      const { writer, reader } = open(session, socket);
      const read = observe(reader.read());
      const closed = observe(writer.closed);
      if (defect === "gap") chunk(socket, 2n);
      if (defect === "accepted") {
        socket.deliver({
          form: "response",
          target: TARGET,
          correlation: 1n,
          envelope: { seq: 1n, stamp: 0n },
          outcome: { outcome: "accepted" },
        });
      }
      if (defect === "upload-chunk") {
        socket.deliver({
          form: "chunk",
          target: TARGET,
          correlation: 1n,
          stream: 1,
          envelope: { seq: 1n, stamp: 0n },
          payload: bytes,
        });
      }
      if (defect === "size") chunk(socket, 1n, new Uint8Array(65536));
      if (defect === "credit") {
        for (let i = 1n; i <= 6n; i++) chunk(socket, i);
      }
      if (defect === "direction") {
        socket.deliver({
          form: "stream_credit",
          target: TARGET,
          correlation: 1n,
          stream: 0,
          credit: 1,
        });
      }
      if (defect === "late-response") chunk(socket);
      if (defect === "response" || defect === "late-response") {
        socket.deliver({
          form: "response",
          target: TARGET,
          correlation: 1n,
          envelope: { seq: 1n, stamp: 0n },
          outcome: defect === "response"
            ? { outcome: "reply", payload: bytes }
            : { outcome: "busy" },
        });
      }
      await tick();
      assert.equal(closed.settled, true);
      if (defect === "gap" || defect === "size") {
        category(read.error, defect === "gap" ? "interrupted" : "corrupt");
        assert.equal(closed.error, read.error);
        assert.ok(
          sent(socket).some((f) =>
            f.form === "stream_end" && f.stream === 0 &&
            f.outcome.kind === (defect === "gap" ? "interrupted" : "corrupt")
          ),
        );
      } else {
        assert.ok(
          socket.closes.length > 0,
          "invalid reply form or excess credit closes the session",
        );
      }
    } finally {
      session.close();
    }
  }
});

Deno.test("duplex_upload_validation", async () => {
  for (
    const failure of [
      "size",
      "request",
      "chunk",
      "end",
      "credit",
      "stop",
    ] as const
  ) {
    const { session, socket } = await rig();
    try {
      const exception = new Error("send failed");
      const send = socket.send.bind(socket);
      if (failure === "request") {
        socket.send = () => {
          throw exception;
        };
      }
      const { upload, stream } = session.queryDuplex(1, 14, bytes, {
        bound: 1000,
      });
      const writer = upload.getWriter();
      const reader = stream.getReader();
      const uploadClosed = observe(writer.closed);
      const replyClosed = observe(reader.closed);
      const read = observe(reader.read());
      if (failure !== "request") {
        assert.equal(
          sent(socket).filter((f) => f.form === "request").length,
          1,
        );
        if (failure !== "size") {
          socket.send = () => {
            throw exception;
          };
        }
        const operation = observe(
          failure === "end"
            ? writer.close()
            : failure === "stop"
            ? reader.cancel()
            : failure === "credit"
            ? Promise.resolve().then(() => chunk(socket))
            : writer.write(failure === "size" ? new Uint8Array(65536) : bytes),
        );
        await tick();
        assert.equal(operation.settled, true);
      }
      await tick();
      assert.equal(uploadClosed.settled, true);
      if (failure === "size") {
        category(uploadClosed.error, "corrupt");
        const stops = sent(socket).filter((f) => f.form === "stream_end");
        assert.deepEqual(
          stops.map((f) => [f.stream, f.envelope.seq, f.outcome.kind]),
          [[1, 1n, "interrupted"], [0, 1n, "interrupted"]],
          "oversized writes must interrupt upload before reply",
        );
        for (const stop of stops) {
          assert.deepEqual(stop.target, TARGET);
          assert.equal(stop.correlation, 1n);
        }
      } else assert.equal(uploadClosed.error, exception);
      if (failure !== "stop") {
        assert.equal(read.settled, true);
        assert.equal(
          failure === "credit" ? replyClosed.error : read.error,
          uploadClosed.error,
        );
      }
      socket.send = send;
      const count = sent(socket).length;
      credit(socket);
      await tick();
      assert.equal(
        sent(socket).length,
        count,
        "failed call sends nothing further",
      );
    } finally {
      session.close();
    }
  }
});

Deno.test("duplex_bound", async () => {
  for (const started of [false, true]) {
    const { session, socket } = await rig();
    try {
      const { writer, reader } = open(session, socket, 20);
      const closed = observe(writer.closed);
      const read = observe(reader.read());
      await writer.write(bytes);
      credit(socket);
      if (started) {
        chunk(socket);
        await tick();
        await writer.close();
      }
      await new Promise((resolve) => setTimeout(resolve, 50));
      if (started) {
        assert.deepEqual(read.value, { done: false, value: bytes });
        const ending = observe(reader.read());
        end(socket, 0, 2n);
        await tick();
        assert.equal(ending.error, undefined);
        assert.equal(ending.value?.done, true);
        assert.equal(
          sent(socket).filter((f) => f.form === "stream_end").length,
          1,
          "close must not restart the reply bound",
        );
      } else {
        assert.equal(
          read.settled,
          true,
          "request starts the first-reply bound",
        );
        category(read.error, "timeout");
        assert.equal(closed.error, read.error);
      }
    } finally {
      session.close();
    }
  }
});

Deno.test("duplex_bound_after_upload_close", async () => {
  const { session, socket } = await rig();
  try {
    const { writer, reader } = open(session, socket, 20);
    const read = observe(reader.read());
    await writer.close();
    await new Promise((resolve) => setTimeout(resolve, 50));
    assert.equal(
      read.settled,
      true,
      "upload close must preserve the first-reply bound",
    );
    category(read.error, "timeout");
    assert.deepEqual(
      sent(socket).filter((f) => f.form === "stream_end").map((f) => [
        f.stream,
        f.outcome.kind,
      ]),
      [[1, "end"], [0, "interrupted"]],
    );
  } finally {
    session.close();
  }
});

Deno.test("duplex_preflight", async () => {
  for (
    const invalid of [
      "v1",
      "v2",
      "iface",
      "ord",
      "bound",
      "args",
      "down",
    ] as const
  ) {
    const { session, socket } = await rig(
      invalid === "v1"
        ? SUBPROTOCOL_V1
        : invalid === "v2"
        ? SUBPROTOCOL
        : SUBPROTOCOL_V3,
    );
    try {
      if (invalid === "down") session.close();
      const before = sent(socket).length;
      const { upload, stream } = session.queryDuplex(
        invalid === "iface" ? 0 : 1,
        invalid === "ord" ? 0 : 14,
        invalid === "args" ? new Uint8Array(65536) : bytes,
        { bound: invalid === "bound" ? 0 : 1000 },
      );
      const writer = upload.getWriter();
      const reader = stream.getReader();
      const write = observe(writer.closed);
      const read = observe(reader.read());
      reader.closed.catch(() => {});
      await tick();
      assert.equal(write.settled, true);
      assert.equal(read.settled, true);
      assert.equal(write.error, read.error);
      if (["iface", "ord", "bound"].includes(invalid)) {
        assert.ok(read.error instanceof RangeError);
      } else {category(
          read.error,
          invalid === "v1" || invalid === "v2"
            ? "unsupported"
            : invalid === "args"
            ? "corrupt"
            : "down",
        );}
      assert.equal(sent(socket).length, before);
    } finally {
      session.close();
    }
  }
});

// Mutation: use the default ceiling instead of the catalog member limit.
Deno.test("duplex_catalog_chunk_limit", async () => {
  for (const direction of ["upload", "reply"] as const) {
    const { session, socket, notes } = await rig(SUBPROTOCOL_V3, {
      interfaces: [{
        iface: 1,
        narrow: { iface: 1, ord: 14, kind: "query", limit: 5 },
      }],
    });
    try {
      const { writer, reader } = open(session, socket);
      const closed = observe(writer.closed);
      const read = direction === "upload" ? observe(reader.read()) : undefined;
      const exact = new Uint8Array(5).fill(7);
      if (direction === "upload") {
        await writer.write(exact);
        assert.deepEqual(
          sent(socket).filter((f) => f.form === "chunk").map((f) => f.payload),
          [exact],
        );
        const operation = observe(writer.write(new Uint8Array(6)));
        await tick();
        assert.equal(operation.settled, true);
        category(operation.error, "corrupt");
      } else {
        chunk(socket, 1n, exact);
        assert.deepEqual(await reader.read(), { done: false, value: exact });
        chunk(socket, 2n, new Uint8Array(6));
      }
      await tick();
      assert.equal(closed.settled, true);
      category(closed.error, "corrupt");
      if (read) {
        assert.equal(read.settled, true);
        assert.equal(read.error, closed.error);
      } else {
        await assert.rejects(
          () => reader.read(),
          (error) => error === closed.error,
        );
      }
      chunk(socket);
      assert.ok(
        notes.some((n) =>
          n.note === "discarded" && n.reason === "unknown-correlation"
        ),
      );
    } finally {
      session.close();
    }
  }
});

// Mutations: skip reply-end sequencing or accept any upload-end sequence.
Deno.test("duplex_end_sequences", async () => {
  for (const direction of ["reply", "upload"] as const) {
    const { session, socket, notes } = await rig();
    try {
      const { writer, reader } = open(session, socket);
      const closed = observe(writer.closed);
      const read = observe(reader.read());
      end(
        socket,
        direction === "reply" ? 0 : 1,
        2n,
        direction === "reply" ? "end" : "busy",
      );
      await tick();
      assert.equal(closed.settled, true);
      category(closed.error, "interrupted");
      assert.equal(read.settled, true);
      assert.equal(read.error, closed.error);
      chunk(socket);
      assert.ok(
        notes.some((n) =>
          n.note === "discarded" && n.reason === "unknown-correlation"
        ),
      );
    } finally {
      session.close();
    }
  }
});

// Advance the caller counter independently of the upload and reply counters.
async function secondCall(session: Session, socket: FakeSocket, bound = 1000) {
  const first = open(session, socket);
  end(socket);
  assert.equal((await first.reader.read()).done, true);
  const { upload, stream } = session.queryDuplex(1, 14, bytes, { bound });
  const writer = upload.getWriter();
  const reader = stream.getReader();
  writer.closed.catch(() => {});
  reader.closed.catch(() => {});
  await writer.write(bytes);
  await writer.write(bytes);
  return { writer, reader };
}

// Mutation: use the upload sequence as the stop correlation at each stop site.
for (const cause of ["size", "cancel", "bound", "gap", "reply-size"] as const) {
  Deno.test(`duplex_stop_correlation_${cause}`, async () => {
    const { session, socket, notes } = await rig();
    try {
      const { writer, reader } = await secondCall(
        session,
        socket,
        cause === "bound" ? 20 : 1000,
      );
      const closed = observe(writer.closed);
      if (cause === "size") {
        const write = observe(writer.write(new Uint8Array(65536)));
        await tick();
        assert.equal(write.settled, true);
        category(write.error, "corrupt");
      } else if (cause === "cancel") await reader.cancel();
      else if (cause === "bound") {
        await new Promise((resolve) => setTimeout(resolve, 50));
      } else {
        socket.deliver({
          form: "chunk",
          target: TARGET,
          correlation: 2n,
          stream: 0,
          envelope: { stamp: 0n, seq: cause === "gap" ? 2n : 1n },
          payload: cause === "gap" ? bytes : new Uint8Array(65536),
        });
      }
      await tick();
      assert.equal(closed.settled, true);
      category(
        closed.error,
        cause === "size" || cause === "reply-size"
          ? "corrupt"
          : cause === "bound"
          ? "timeout"
          : "interrupted",
      );
      const stops = sent(socket).filter((f) => f.form === "stream_end");
      assert.deepEqual(
        stops.map((f) => [
          f.target,
          f.correlation,
          f.stream,
          f.envelope.seq,
          f.outcome.kind,
        ]),
        cause === "size"
          ? [
            [TARGET, 2n, 1, 3n, "interrupted"],
            [TARGET, 2n, 0, 1n, "interrupted"],
          ]
          : [[
            TARGET,
            2n,
            0,
            1n,
            cause === "reply-size" ? "corrupt" : "interrupted",
          ]],
      );
      socket.deliver({
        form: "chunk",
        target: TARGET,
        correlation: 2n,
        stream: 0,
        envelope: { stamp: 0n, seq: 1n },
        payload: bytes,
      });
      assert.ok(
        notes.some((n) =>
          n.note === "discarded" && n.reason === "unknown-correlation"
        ),
      );
    } finally {
      session.close();
    }
  });
}

// Mutation: bypass #send for the stream-0 stop after an oversized write.
Deno.test("duplex_oversized_stop_send_exception", async () => {
  const { session, socket, notes } = await rig();
  try {
    const { writer, reader } = await secondCall(session, socket);
    const exception = new Error("reply interruption send failed");
    const send = socket.send.bind(socket);
    socket.send = (data) => {
      assert.ok(data instanceof Uint8Array);
      const decoded = decode(data);
      assert.ok(decoded.ok);
      if (decoded.frame.form === "stream_end" && decoded.frame.stream === 0) {
        throw exception;
      }
      send(data);
    };
    const closed = observe(writer.closed);
    const read = observe(reader.read());
    const replyClosed = observe(reader.closed);
    const write = observe(writer.write(new Uint8Array(65536)));
    await tick();
    assert.equal(write.settled, true);
    assert.equal(write.error, exception);
    assert.equal(closed.settled, true);
    assert.equal(closed.error, exception);
    assert.equal(read.settled, true, "the reply must reject immediately");
    assert.equal(read.error, exception);
    assert.equal(replyClosed.settled, true);
    assert.equal(replyClosed.error, exception);
    assert.deepEqual(
      sent(socket).filter((f) => f.form === "stream_end").map((f) => [
        f.correlation,
        f.stream,
        f.envelope.seq,
        f.outcome.kind,
      ]),
      [[2n, 1, 3n, "interrupted"]],
    );
    socket.send = send;
    socket.deliver({
      form: "chunk",
      target: TARGET,
      correlation: 2n,
      stream: 0,
      envelope: { stamp: 0n, seq: 1n },
      payload: bytes,
    });
    assert.ok(
      notes.some((n) =>
        n.note === "discarded" && n.reason === "unknown-correlation"
      ),
      "the failed call must be forgotten",
    );
    assert.equal(
      socket.closes.length,
      0,
      "send failure leaves the session open",
    );
  } finally {
    session.close();
  }
});
