import assert from "node:assert/strict";
import {
  BLOCK_BYTES,
  connect,
  decode,
  Kind,
  SessionError,
  SUBPROTOCOL,
} from "../mod.ts";
import type {
  ConnectOptions,
  Frame,
  Note,
  Target,
  WaitOptions,
} from "../mod.ts";
import { FakeSocket } from "./fake_socket.ts";

type Options = WaitOptions;
const SIGNAL: Target = { kind: Kind.Signal, iface: 1, ord: 2 };
const EVENT: Target = { kind: Kind.Event, iface: 1, ord: 3 };
const FIXED: Target = { kind: Kind.Fixed, iface: 1, ord: 4 };
const LONG = { bound: 2_147_483_647 };

const abortListeners: Set<EventListenerOrEventListenerObject>[] = [];

/** Records listener registration and removal while using a real signal. */
function trackedController(): AbortController {
  const controller = new AbortController();
  const signal = controller.signal;
  const listeners = new Set<EventListenerOrEventListenerObject>();
  abortListeners.push(listeners);
  const add = signal.addEventListener.bind(signal);
  const remove = signal.removeEventListener.bind(signal);
  signal.addEventListener = (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | AddEventListenerOptions,
  ) => {
    if (type === "abort") listeners.add(listener);
    add(type, listener, options);
  };
  signal.removeEventListener = (
    type: string,
    listener: EventListenerOrEventListenerObject,
    options?: boolean | EventListenerOptions,
  ) => {
    if (type === "abort") listeners.delete(listener);
    remove(type, listener, options);
  };
  return controller;
}

// A timer or abort listener left running must fail the test.
function test(name: string, fn: () => Promise<void>): void {
  Deno.test({
    name,
    sanitizeOps: true,
    fn: async () => {
      try {
        await fn();
        for (const listeners of abortListeners) assert.equal(listeners.size, 0);
      } finally {
        abortListeners.length = 0;
      }
    },
  });
}
function start(options: Partial<ConnectOptions> & Options = {}) {
  const sockets: FakeSocket[] = [];
  const notes: Note[] = [];
  const pending = connect("ws://example.test", {
    catalog: { name: "test.wait", hash: new Uint8Array(32) },
    onNote: (note) => notes.push(note),
    webSocket: (url, protocols) => {
      const socket = new FakeSocket(url, protocols);
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
    ...options,
  });
  return { sockets, pending, notes };
}
async function attached(options: Options = {}) {
  const h = start(options);
  const socket = h.sockets[0];
  socket.open(SUBPROTOCOL);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  return { ...h, socket, session: await h.pending };
}
function frames(socket: FakeSocket): Frame[] {
  return socket.sent.map((bytes) => {
    const result = decode(bytes);
    assert.ok(result.ok);
    return result.frame;
  });
}
function accepted(target = SIGNAL): Frame {
  return { form: "answer", target, outcome: { outcome: "accepted" } };
}
function refused(target: Target): Frame {
  return {
    form: "answer",
    target,
    outcome: {
      outcome: "contract",
      contract: { category: "unknown-interaction" },
    },
  };
}
function publish(target = SIGNAL, seq = 1n): Frame {
  return {
    form: "publish",
    target,
    envelope: { stamp: seq, seq },
    state: { provenance: "live", payload: Uint8Array.of(Number(seq)) },
  };
}
function failure(category: string) {
  return (e: unknown) =>
    e instanceof SessionError && e.failure.category === category;
}

for (const bound of [0, NaN, 2 ** 31, Infinity, -1]) {
  test(`connect rejects invalid bound ${bound} before creating a socket`, async () => {
    const h = start({ bound });
    const rejected = assert.rejects(h.pending, RangeError);
    if (h.sockets.length) h.sockets[0].error();
    await rejected;
    assert.equal(h.sockets.length, 0);
  });
}
for (const opened of [false, true]) {
  test(`connect timeout closes the socket and ignores late events (opened=${opened})`, async () => {
    const controller = trackedController();
    const h = start({ bound: 10, signal: controller.signal });
    const socket = h.sockets[0];
    if (opened) socket.open(SUBPROTOCOL);
    await assert.rejects(h.pending, failure("timeout"));
    assert.equal(socket.closes.length, 1);
    controller.abort("after timeout");
    assert.equal(socket.closes.length, 1);
    const sent = socket.sent.length;
    socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
    socket.open(SUBPROTOCOL);
    socket.serverClose(1000);
    assert.equal(socket.sent.length, sent);
    assert.deepEqual(h.notes, []);
  });
}
test("connect abort rejects with the exact reason and ignores late events", async () => {
  const controller = trackedController();
  const reason = { cancelled: true };
  const h = start({ ...LONG, signal: controller.signal });
  const rejected = assert.rejects(h.pending, (e) => e === reason);
  controller.abort(reason);
  await rejected;
  const socket = h.sockets[0];
  assert.equal(socket.closes.length, 1);
  socket.open(SUBPROTOCOL);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  socket.serverClose(1000);
  assert.equal(socket.sent.length, 0);
});
test("an already aborted connect creates no socket", async () => {
  const controller = trackedController();
  controller.abort("stop");
  const h = start({ signal: controller.signal });
  await assert.rejects(h.pending, (e) => e === "stop");
  assert.equal(h.sockets.length, 0);
});
test("connect success clears its timer and abort listener", async () => {
  const controller = trackedController();
  const h = await attached({ ...LONG, signal: controller.signal });
  controller.abort("after success");
  assert.equal(h.socket.closes.length, 0);
  h.session.close();
});
test("connect bound includes time spent creating the socket", async () => {
  const originalNow = performance.now;
  const originalSet = globalThis.setTimeout;
  const originalClear = globalThis.clearTimeout;
  let now = 1000;
  let timer: { at: number; fire: () => void } | undefined;
  const advance = (milliseconds: number) => {
    now += milliseconds;
    if (timer && timer.at <= now) {
      const fire = timer.fire;
      timer = undefined;
      fire();
    }
  };
  performance.now = () => now;
  globalThis.setTimeout = ((fire: () => void, delay = 0) => {
    timer = { at: now + delay, fire };
    return 1;
  }) as unknown as typeof setTimeout;
  globalThis.clearTimeout = () => {
    timer = undefined;
  };
  const socket = new FakeSocket("ws://example.test", []);
  try {
    const h = start({
      bound: 100,
      webSocket: () => {
        advance(40);
        return socket as unknown as WebSocket;
      },
    });
    let settled = false;
    let error: unknown;
    const result = h.pending.then(
      () => {
        settled = true;
      },
      (reason) => {
        settled = true;
        error = reason;
      },
    );
    advance(59);
    await Promise.resolve();
    assert.equal(settled, false);
    assert.equal(socket.closes.length, 0);
    advance(1);
    await Promise.resolve();
    await Promise.resolve();
    assert.equal(settled, true);
    await result;
    assert.ok(failure("timeout")(error));
    assert.equal(socket.closes.length, 1);
    assert.equal(timer, undefined);
  } finally {
    socket.error();
    performance.now = originalNow;
    globalThis.setTimeout = originalSet;
    globalThis.clearTimeout = originalClear;
  }
});
for (const ending of ["refused", "down", "violation"] as const) {
  test(`connect clears its timer on ${ending}`, async () => {
    const controller = trackedController();
    const h = start({ ...LONG, signal: controller.signal });
    const rejected = assert.rejects(h.pending);
    const socket = h.sockets[0];
    socket.open(SUBPROTOCOL);
    if (ending === "down") socket.error();
    else if (ending === "violation") socket.deliver("bad");
    else {socket.deliver({
        form: "attached",
        outcome: { outcome: "refused", reason: "unknown-catalog" },
      });}
    await rejected;
    socket.serverClose(1000);
  });
}
for (const op of ["subscribe", "read"] as const) {
  test(`${op} removes the original signal listener after options change`, async () => {
    const h = await attached();
    try {
      const first = trackedController();
      const second = trackedController();
      const options = { signal: first.signal };
      const pending = h.session[op](SIGNAL, options);
      options.signal = second.signal;
      h.socket.deliver(op === "subscribe" ? accepted() : publish());
      await pending;
    } finally {
      h.session.close();
    }
  });
  test(`${op} rejects with the original signal reason after options change`, async () => {
    const h = await attached();
    try {
      const first = trackedController();
      const second = trackedController();
      const reason = { cancelled: "original signal" };
      const options = { signal: first.signal };
      const rejected = assert.rejects(
        h.session[op](SIGNAL, options),
        (error) => error === reason,
      );
      options.signal = second.signal;
      first.abort(reason);
      await rejected;
    } finally {
      h.session.close();
    }
  });
  test(`${op} send failure leaves no placeholder ahead of later replies`, async () => {
    const h = await attached();
    try {
      const first = h.session[op](SIGNAL, LONG);
      const send = h.socket.send.bind(h.socket);
      const reason = new Error("send failed");
      h.socket.send = () => {
        throw reason;
      };
      const controller = trackedController();
      await assert.rejects(
        h.session[op](SIGNAL, { ...LONG, signal: controller.signal }),
        (error) => error === reason,
      );
      h.socket.send = send;
      let settled = false;
      const next = h.session[op](SIGNAL, LONG).then(() => {
        settled = true;
      });
      next.catch(() => {});
      h.socket.deliver(op === "subscribe" ? accepted() : publish());
      if (op === "subscribe") h.socket.deliver(accepted());
      await first;
      assert.equal(settled, true);
      await next;
      assert.deepEqual(h.notes, []);
    } finally {
      h.session.close();
    }
  });
  test(`${op} validates bound and pre-aborted signal before sending`, async () => {
    const h = await attached();
    const before = h.socket.sent.length;
    for (const bound of [0, NaN, 2 ** 31, Infinity, -1]) {
      await assert.rejects(h.session[op](SIGNAL, { bound }), RangeError);
    }
    const controller = trackedController();
    controller.abort("already stopped");
    await assert.rejects(
      h.session[op](SIGNAL, { signal: controller.signal }),
      (e) => e === "already stopped",
    );
    assert.equal(h.socket.sent.length, before);
    h.session.close();
  });
  for (const ending of ["accepted", "contract", "down"] as const) {
    test(`${op} clears timer and listener on ${ending}`, async () => {
      const h = await attached();
      const controller = trackedController();
      const pending = h.session[op](SIGNAL, {
        ...LONG,
        signal: controller.signal,
      });
      const result = ending === "accepted"
        ? pending
        : assert.rejects(pending, failure(ending));
      if (ending === "down") h.socket.serverClose(1000);
      else if (ending === "contract") h.socket.deliver(refused(SIGNAL));
      else h.socket.deliver(op === "subscribe" ? accepted() : publish());
      await result;
      const sent = h.socket.sent.length;
      controller.abort("after settlement");
      assert.equal(h.socket.sent.length, sent);
      h.session.close();
    });
  }
}
for (const mode of ["timeout", "abort"] as const) {
  const options = (controller: AbortController): Options => ({
    bound: mode === "timeout" ? 10 : LONG.bound,
    signal: controller.signal,
  });
  const rejects = (promise: Promise<unknown>) =>
    assert.rejects(
      promise,
      mode === "timeout" ? failure("timeout") : (e) => e === "stop",
    );
  for (const target of [SIGNAL, EVENT]) {
    test(`subscribe ${mode} unsubscribes and absorbs late acceptance for kind ${target.kind}`, async () => {
      const h = await attached();
      const controller = trackedController();
      const rejected = rejects(
        h.session.subscribe(target, options(controller)),
      );
      if (mode === "abort") controller.abort("stop");
      await rejected;
      assert.deepEqual(frames(h.socket).at(-1), {
        form: "unsubscribe",
        target,
      });
      const sent = h.socket.sent.length;
      controller.abort("late abort");
      assert.equal(h.socket.sent.length, sent);
      h.socket.deliver(accepted(target));
      h.socket.deliver(publish(target));
      assert.equal(h.session.sample(target.iface, target.ord), undefined);
      assert.ok(
        !h.notes.some((n) =>
          n.note === "discarded" && n.reason === "unexpected-answer"
        ),
      );
      h.session.close();
    });
  }
  test(`subscribe ${mode} preserves a live subscribe behind the placeholder`, async () => {
    const h = await attached();
    const controller = trackedController();
    const rejected = rejects(h.session.subscribe(SIGNAL, options(controller)));
    let settled = false;
    const second = h.session.subscribe(SIGNAL, LONG).then(() => settled = true);
    if (mode === "abort") controller.abort("stop");
    await rejected;
    assert.ok(!frames(h.socket).some((f) => f.form === "unsubscribe"));
    h.socket.deliver(accepted());
    await Promise.resolve();
    assert.equal(settled, false);
    h.socket.deliver(publish());
    assert.equal(h.session.sample(1, 2), undefined);
    h.socket.deliver(accepted());
    await second;
    h.socket.deliver(publish());
    assert.equal(h.session.sample(1, 2)?.envelope.seq, 1n);
    h.session.close();
  });
  for (const reply of ["publish", "contract", "blocks", "collected"] as const) {
    test(`read ${mode} absorbs late ${reply} before the next read`, async () => {
      const h = await attached();
      const controller = trackedController();
      const rejected = rejects(h.session.read(FIXED, options(controller)));
      const block = (index: number): Frame => ({
        form: "block",
        target: FIXED,
        envelope: { stamp: 1n, seq: 0n },
        total: BLOCK_BYTES + 1,
        index,
        payload: new Uint8Array(index === 0 ? BLOCK_BYTES : 1),
      });
      if (reply === "collected") h.socket.deliver(block(0));
      let settled = false;
      const second = h.session.read(FIXED, LONG).then((sample) => {
        settled = true;
        return sample;
      });
      if (mode === "abort") controller.abort("stop");
      await rejected;
      if (reply === "publish") h.socket.deliver(publish(FIXED));
      else if (reply === "contract") h.socket.deliver(refused(FIXED));
      else {
        if (reply === "blocks") h.socket.deliver(block(0));
        h.socket.deliver(block(1));
      }
      await Promise.resolve();
      assert.equal(settled, false);
      h.socket.deliver(publish(FIXED, 2n));
      assert.equal((await second).envelope.seq, 2n);
      assert.deepEqual(h.notes, []);
      h.session.close();
    });
  }
}
test("a cancelled duplicate subscribe preserves held sample and event sequence", async () => {
  const h = await attached();
  for (const target of [SIGNAL, EVENT]) {
    const subscribed = h.session.subscribe(target);
    h.socket.deliver(accepted(target));
    await subscribed;
    if (target === SIGNAL) h.socket.deliver(publish());
    else {h.socket.deliver({
        form: "occurrence",
        target,
        envelope: { stamp: 1n, seq: 1n },
        payload: new Uint8Array(),
      });}
    const controller = trackedController();
    const rejected = assert.rejects(
      h.session.subscribe(target, { signal: controller.signal }),
      (e) => e === "stop",
    );
    const sent = h.socket.sent.length;
    controller.abort("stop");
    await rejected;
    assert.equal(h.socket.sent.length, sent);
    h.socket.deliver(accepted(target));
  }
  assert.equal(h.session.sample(1, 2)?.envelope.seq, 1n);
  h.socket.deliver({
    form: "occurrence",
    target: EVENT,
    envelope: { stamp: 3n, seq: 3n },
    payload: new Uint8Array(),
  });
  assert.ok(h.notes.some((n) => n.note === "loss" && n.expected === 2n));
  h.session.close();
});

test("a cancelled read in the middle keeps the replies of both live reads separate", async () => {
  const h = await attached();
  const first = h.session.read(FIXED, LONG);
  const controller = trackedController();
  const rejected = assert.rejects(
    h.session.read(FIXED, { signal: controller.signal }),
    (e) => e === "stop",
  );
  let settled = false;
  const third = h.session.read(FIXED, LONG).then((sample) => {
    settled = true;
    return sample;
  });
  controller.abort("stop");
  await rejected;
  h.socket.deliver(publish(FIXED, 1n));
  assert.equal((await first).envelope.seq, 1n);
  assert.equal(settled, false);
  h.socket.deliver(publish(FIXED, 2n));
  await Promise.resolve();
  assert.equal(settled, false);
  h.socket.deliver(publish(FIXED, 3n));
  assert.equal((await third).envelope.seq, 3n);
  assert.deepEqual(h.notes, []);
  h.session.close();
});

test("cancelling the last live subscribe sends unsubscribe", async () => {
  const h = await attached();
  const first = trackedController();
  const second = trackedController();
  const firstRejected = assert.rejects(
    h.session.subscribe(SIGNAL, { signal: first.signal }),
    (e) => e === "first",
  );
  const secondRejected = assert.rejects(
    h.session.subscribe(SIGNAL, { signal: second.signal }),
    (e) => e === "second",
  );
  first.abort("first");
  await firstRejected;
  assert.ok(!frames(h.socket).some((f) => f.form === "unsubscribe"));
  second.abort("second");
  await secondRejected;
  assert.equal(
    frames(h.socket).filter((f) => f.form === "unsubscribe").length,
    1,
  );
  h.socket.deliver(accepted());
  h.socket.deliver(accepted());
  h.socket.deliver(publish());
  assert.equal(h.session.sample(1, 2), undefined);
  assert.ok(
    !h.notes.some((n) =>
      n.note === "discarded" && n.reason === "unexpected-answer"
    ),
  );
  h.session.close();
});

test("connect handles abort inside the socket factory without sending attach", async () => {
  const controller = trackedController();
  const socket = new FakeSocket("ws://example.test", []);
  const reason = { cancelled: "during creation" };
  const h = start({
    ...LONG,
    signal: controller.signal,
    webSocket: () => {
      controller.abort(reason);
      return socket as unknown as WebSocket;
    },
  });
  await assert.rejects(h.pending, (error) => error === reason);
  assert.equal(socket.closes.length, 1);
  socket.open(SUBPROTOCOL);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  assert.equal(socket.sent.length, 0);
});

test("connect cancellation falls back to close without a code before open", async () => {
  const controller = trackedController();
  const h = start({ ...LONG, signal: controller.signal });
  const socket = h.sockets[0];
  const close = socket.close.bind(socket);
  socket.close = (code, reason) => {
    if (code !== undefined) throw new Error("close code is unavailable");
    close(code, reason);
  };
  const rejected = assert.rejects(h.pending, (error) => error === "stop");
  controller.abort("stop");
  await rejected;
  assert.deepEqual(socket.closes, [{}]);
  socket.open(SUBPROTOCOL);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  assert.equal(socket.sent.length, 0);
});

for (const mode of ["timeout", "abort"] as const) {
  test(`resubscribe ${mode} unsubscribes despite an earlier dropped waiter`, async () => {
    const h = await attached();
    const first = h.session.subscribe(SIGNAL, LONG);
    h.session.unsubscribe(SIGNAL);
    const controller = trackedController();
    const rejected = assert.rejects(
      h.session.subscribe(SIGNAL, {
        bound: mode === "timeout" ? 10 : LONG.bound,
        signal: controller.signal,
      }),
      mode === "timeout" ? failure("timeout") : (error) => error === "stop",
    );
    if (mode === "abort") controller.abort("stop");
    await rejected;
    const unsubscribes = frames(h.socket).filter((f) =>
      f.form === "unsubscribe"
    );
    // Complete the old waiter even if the cleanup assertion fails.
    h.socket.deliver(accepted());
    await first;
    h.socket.deliver(accepted());
    h.session.close();
    assert.equal(unsubscribes.length, 2);
  });
}
