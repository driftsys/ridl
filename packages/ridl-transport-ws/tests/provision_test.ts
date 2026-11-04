import assert from "node:assert/strict";
import {
  decode,
  encode,
  Kind,
  ProtocolError,
  provide,
  SessionError,
} from "../mod.ts";
import type {
  Frame,
  Note,
  ProvidedRequest,
  ProvideOptions,
  ResponseOutcome,
  Target,
} from "../mod.ts";
import { FakeSocket } from "./fake_socket.ts";
import { FIXED_LARGE, IFACE, pattern } from "../e2e/catalog.ts";
import { nextSequence } from "../src/provision_calls.ts";
// alpha.Face in the committed identity fixture: signal 1, event 2, command 3, query 4.
const S: Target = { kind: Kind.Signal, iface: 1, ord: 1 };
const E: Target = { kind: Kind.Event, iface: 1, ord: 2 };
const C: Target = { kind: Kind.Command, iface: 1, ord: 3 };
const Q: Target = { kind: Kind.Query, iface: 1, ord: 4 };
// alpha has no fixed: this target must be refused.
const F: Target = { kind: Kind.Fixed, iface: 1, ord: 5 };
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
const members = [
  { ...S, kind: "signal", limit: 2 },
  { ...E, kind: "event", limit: 2 },
  { ...C, kind: "command", limit: 2 },
  { ...Q, kind: "query", limit: 2 },
] as const;
const bytes = new Uint8Array([42, 7]);
const accepted = {
  form: "attached",
  outcome: { outcome: "accepted" },
} as const;
const unknown = {
  outcome: "contract",
  contract: { category: "unknown-interaction" },
} as const;
const tick = async () => {
  for (let i = 0; i < 8; i++) await Promise.resolve();
};
function start(extra: Partial<ProvideOptions> = {}) {
  let socket!: FakeSocket;
  const notes: Note[] = [], requests: ProvidedRequest[] = [];
  const pending = provide("ws://fixture/alpha", {
    catalog,
    encoding: "flatbuffers",
    members,
    onRequest: (r) => {
      requests.push(r);
      return { outcome: "accepted" };
    },
    onNote: (n) => notes.push(n),
    clock: () => 77n,
    ...extra,
    webSocket: (u, p) => {
      socket = new FakeSocket(u, p);
      return socket as unknown as WebSocket;
    },
  });
  return { socket, notes, requests, pending };
}
function attach(
  h: ReturnType<typeof start>,
  change: Partial<Extract<Frame, { form: "attach" }>> = {},
) {
  h.socket.deliver({
    form: "attach",
    frameVersion: 2,
    encoding: 1,
    ...catalog,
    ...change,
  });
}
async function open(extra: Partial<ProvideOptions> = {}) {
  const h = start(extra);
  h.socket.open("ridl-ws-v2-provider");
  attach(h);
  return { ...h, p: await h.pending };
}
function exact(s: FakeSocket, i: number, f: Frame) {
  assert.deepEqual(s.sent[i], encode(f));
}
function last(s: FakeSocket): Frame {
  const d = decode(s.sent.at(-1)!);
  assert(d.ok);
  return d.frame;
}
function req(target: Target, seq: bigint, payload = bytes): Frame {
  return { form: "request", target, envelope: { seq, stamp: 12n }, payload };
}
function sample(
  target: Target,
  seq: bigint,
  stamp: bigint,
  state: Extract<Frame, { form: "publish" }>["state"],
): Frame {
  return { form: "publish", target, envelope: { seq, stamp }, state };
}
Deno.test("provide_waits_for_and_validates_attach", async () => {
  const h = start();
  let resolved = false;
  const done = h.pending.then(() => {
    exact(h.socket, 0, accepted);
    resolved = true;
  });
  assert.deepEqual(h.socket.protocols, ["ridl-ws-v2-provider"]);
  h.socket.open("ridl-ws-v2-provider");
  await tick();
  assert.equal(resolved, false);
  assert.equal(h.socket.sent.length, 0);
  attach(h);
  await done;
  assert(resolved);
  for (
    const [change, reason] of [
      [{ frameVersion: 3 }, "frame-version-unsupported"],
      [{ name: "beta" }, "unknown-catalog"],
      [{ hash: new Uint8Array(32) }, "catalog-mismatch"],
      [{ encoding: 2 }, "encoding-unsupported"],
    ] as const
  ) {
    const x = start();
    const rejected = assert.rejects(
      x.pending,
      (e: unknown) =>
        e instanceof Error && "reason" in e && e.reason === reason,
    );
    x.socket.open("ridl-ws-v2-provider");
    attach(x, change);
    await rejected;
    exact(x.socket, 0, {
      form: "attached",
      outcome: { outcome: "refused", reason },
    });
    assert.deepEqual(x.socket.closes, [{ code: 1000 }]);
  }
});
Deno.test("provide_requires_selected_provider_protocol", async () => {
  for (const protocol of ["", "ridl-ws-v1", "ridl-ws-v2", "ridl-ws-v3"]) {
    const h = start();
    const rejected = assert.rejects(h.pending, ProtocolError);
    h.socket.open(protocol);
    await rejected;
    assert.deepEqual(h.socket.closes, [{}]);
    assert.deepEqual(h.notes, [{
      note: "violation",
      violation: "no-subprotocol",
    }]);
    assert.equal(h.socket.sent.length, 0);
  }
});
Deno.test("provide_subscription_and_initial_state", async () => {
  let clocks = 0;
  const h = await open({ clock: () => BigInt(++clocks) * 77n });
  h.socket.deliver({ form: "read", target: S });
  exact(h.socket, 1, sample(S, 0n, 77n, { provenance: "init" }));
  h.socket.deliver({ form: "read", target: S });
  exact(h.socket, 2, sample(S, 0n, 77n, { provenance: "init" }));
  assert.equal(clocks, 1);
  h.socket.sent.splice(1);
  h.p.publish(1, 1, bytes, 10n);
  h.p.raise(1, 2, bytes, 10n);
  assert.equal(h.socket.sent.length, 1);
  assert.deepEqual(h.notes, [{
    note: "discarded",
    reason: "not-subscribed",
    frame: {
      form: "occurrence",
      target: E,
      envelope: { seq: 1n, stamp: 10n },
      payload: bytes,
    },
  }]);
  h.socket.deliver({ form: "subscribe", target: S });
  exact(h.socket, 1, {
    form: "answer",
    target: S,
    outcome: { outcome: "accepted" },
  });
  exact(
    h.socket,
    2,
    sample(S, 1n, 10n, { provenance: "live", payload: bytes }),
  );
  h.socket.deliver({ form: "unsubscribe", target: S });
  h.p.invalidate(1, 1, 11n);
  assert.equal(h.socket.sent.length, 3);
  h.socket.deliver({ form: "subscribe", target: S });
  exact(h.socket, 3, {
    form: "answer",
    target: S,
    outcome: { outcome: "accepted" },
  });
  exact(h.socket, 4, sample(S, 2n, 11n, { provenance: "invalid" }));
});
Deno.test("provide_encodes_values_events_and_invalidation", async () => {
  const h = await open();
  for (const target of [S, E]) h.socket.deliver({ form: "subscribe", target });
  h.p.publish(1, 1, bytes, -8n);
  h.p.raise(1, 2, bytes, 123n);
  h.p.invalidate(1, 1, -8n);
  h.p.raise(1, 2, new Uint8Array(), 124n);
  exact(
    h.socket,
    4,
    sample(S, 1n, -8n, { provenance: "live", payload: bytes }),
  );
  exact(h.socket, 5, {
    form: "occurrence",
    target: E,
    envelope: { seq: 1n, stamp: 123n },
    payload: bytes,
  });
  exact(h.socket, 6, sample(S, 2n, -8n, { provenance: "invalid" }));
  exact(h.socket, 7, {
    form: "occurrence",
    target: E,
    envelope: { seq: 2n, stamp: 124n },
    payload: new Uint8Array(),
  });
  assert.equal(nextSequence(0xffff_ffff_ffff_ffffn), 0n);
  assert.equal(nextSequence(0n), 1n);
  h.socket.deliver({ form: "unsubscribe", target: E });
  assert.doesNotThrow(() => h.p.raise(1, 2, bytes, 0n));
  h.socket.deliver({ form: "subscribe", target: E });
  assert.equal(h.socket.sent.length, 9);
  h.p.raise(1, 2, bytes, 125n);
  exact(h.socket, 9, {
    form: "occurrence",
    target: E,
    envelope: { seq: 4n, stamp: 125n },
    payload: bytes,
  });
});
Deno.test("provide_dispatches_and_replays_requests_once", async () => {
  let settle!: (r: ResponseOutcome) => void;
  const h = await open({
    onRequest: (r) => {
      h.requests.push(r);
      return new Promise((resolve) => {
        settle = resolve;
      });
    },
  });
  h.socket.deliver(req(C, 1n));
  await tick();
  assert.deepEqual(h.requests, [{
    target: C,
    envelope: { seq: 1n, stamp: 12n },
    correlation: 1n,
    payload: bytes,
  }]);
  h.socket.deliver(req(C, 1n));
  await tick();
  assert.equal(h.requests.length, 1);
  assert.equal(h.socket.sent.length, 1);
  settle({ outcome: "accepted" });
  await tick();
  exact(h.socket, 1, {
    form: "response",
    target: C,
    envelope: { seq: 1n, stamp: 77n },
    correlation: 1n,
    outcome: { outcome: "accepted" },
  });
  h.socket.deliver(req(C, 1n));
  await tick();
  assert.equal(h.requests.length, 1);
  assert.deepEqual(h.socket.sent[1], h.socket.sent[2]);
  h.socket.deliver(req(Q, 2n));
  await tick();
  settle({ outcome: "reply", payload: bytes });
  await tick();
  exact(h.socket, 3, {
    form: "response",
    target: Q,
    envelope: { seq: 2n, stamp: 77n },
    correlation: 2n,
    outcome: { outcome: "reply", payload: bytes },
  });
  h.socket.deliver(req(C, 3n));
  await tick();
  settle(unknown);
  await tick();
  exact(h.socket, 4, {
    form: "response",
    target: C,
    envelope: { seq: 3n, stamp: 77n },
    correlation: 3n,
    outcome: unknown,
  });
});
Deno.test("provide_bounds_and_bad_direction", async () => {
  const h = await open();
  h.socket.deliver({ form: "subscribe", target: S });
  assert.throws(() => h.p.publish(1, 1, new Uint8Array(3), 0n), SessionError);
  assert.throws(() => h.p.publish(1, 2, bytes, 0n), SessionError);
  assert.throws(() => h.p.publish(0, 1, bytes, 0n), RangeError);
  assert.throws(() => h.p.publish(1, 1, bytes, 1n << 63n), RangeError);
  for (const target of [F, C, { ...S, kind: Kind.Event }]) {
    h.socket.deliver({ form: "subscribe", target });
    exact(h.socket, h.socket.sent.length - 1, {
      form: "answer",
      target,
      outcome: unknown,
    });
  }
  h.socket.deliver({ form: "read", target: F });
  exact(h.socket, h.socket.sent.length - 1, {
    form: "answer",
    target: F,
    outcome: unknown,
  });
  for (const target of [S, { ...Q, ord: 3 }, { ...C, iface: 2 }]) {
    h.socket.deliver(req(target, BigInt(h.socket.sent.length)));
    await tick();
    const f = last(h.socket);
    assert(f.form === "response");
    assert.deepEqual(f.target, target);
    assert.deepEqual(f.outcome, unknown);
  }
  h.socket.deliver(req(C, 99n, new Uint8Array(3)));
  await tick();
  const f = last(h.socket);
  assert(f.form === "response");
  assert.deepEqual(f.outcome, { outcome: "corrupt" });
  assert.equal(h.requests.length, 0);
  for (
    const message of [
      accepted,
      {
        form: "block",
        target: { kind: Kind.Fixed, iface: IFACE, ord: FIXED_LARGE },
        envelope: { seq: 1n, stamp: 0n },
        total: 65_280,
        index: 0,
        payload: pattern(4096),
      },
      {
        form: "response",
        target: C,
        envelope: { seq: 1n, stamp: 0n },
        correlation: 1n,
        outcome: { outcome: "accepted" },
      },
      {
        form: "occurrence",
        target: E,
        envelope: { seq: 1n, stamp: 0n },
        payload: bytes,
      },
      { form: "answer", target: S, outcome: { outcome: "accepted" } },
      { form: "attach", frameVersion: 2, encoding: 1, ...catalog },
      sample(S, 1n, 0n, { provenance: "init" }),
      new Uint8Array([12]),
      new Uint8Array([13]),
      new Uint8Array([14]),
      "text",
      new Uint8Array([255]),
    ] as const
  ) {
    const x = await open();
    x.socket.deliver(message);
    assert.deepEqual(x.socket.closes, [{}]);
    assert.equal(x.notes[0]?.note, "violation");
  }
});
Deno.test("provide_close_settles_pending_operations", async () => {
  for (const end of ["close", "eof", "error"] as const) {
    let clocks = 0;
    let finish!: (r: ResponseOutcome) => void;
    const h = await open({
      clock: () => {
        clocks++;
        return 77n;
      },
      onRequest: () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    });
    h.socket.deliver(req(C, 1n));
    await tick();
    if (end === "close") h.p.close();
    else if (end === "eof") h.socket.serverClose(1000);
    else h.socket.error();
    finish({ outcome: "accepted" });
    await tick();
    assert.equal(h.socket.sent.length, 1);
    assert.equal(clocks, 1);
    assert.deepEqual(h.notes, []);
    assert.throws(() => h.p.publish(1, 1, bytes, 0n), SessionError);
    h.socket.serverClose(1000);
    await h.p.closed;
  }
  for (
    const fail of [() => {
      throw Error("sync");
    }, () => Promise.reject(Error("async"))]
  ) {
    const h = await open({ onRequest: fail });
    h.socket.deliver(req(C, 1n));
    await tick();
    assert.equal(h.socket.sent.length, 1);
    assert.deepEqual(h.notes, [{
      note: "discarded",
      reason: "unexpected-outcome",
      frame: req(C, 1n),
    }]);
    h.socket.deliver(req(C, 1n));
    await tick();
    assert.equal(h.socket.sent.length, 1);
  }
  const h = start();
  const rejected = assert.rejects(h.pending, SessionError);
  h.socket.error();
  await rejected;
});
Deno.test("provide_request_capacity_retention_and_outcome_validation", async () => {
  const h = await open({
    onRequest: (r) => {
      h.requests.push(r);
      return new Promise(() => {});
    },
  });
  for (let i = 1; i <= 65; i++) h.socket.deliver(req(C, BigInt(i)));
  await tick();
  assert.equal(h.requests.length, 64);
  const busy = last(h.socket);
  assert(busy.form === "response");
  assert.equal(busy.correlation, 65n);
  assert.deepEqual(busy.outcome, { outcome: "busy" });
  const x = await open();
  for (let i = 1; i <= 65; i++) {
    x.socket.deliver(req(C, BigInt(i)));
    await tick();
  }
  const count = x.socket.sent.length;
  x.socket.deliver(req(C, 1n));
  await tick();
  assert.equal(x.requests.length, 65);
  assert.equal(x.socket.sent.length, count);
  x.socket.deliver(req(C, 2n));
  await tick();
  assert.deepEqual(x.socket.sent.at(-1), x.socket.sent[2]);
  for (
    const [target, outcome] of [[C, { outcome: "reply", payload: bytes }], [Q, {
      outcome: "accepted",
    }], [Q, { outcome: "reply", payload: new Uint8Array(3) }]] as const
  ) {
    const y = await open({ onRequest: () => outcome });
    y.socket.deliver(req(target, 1n));
    await tick();
    if (outcome.outcome === "reply" && outcome.payload.length === 3) {
      const f = last(y.socket);
      assert(f.form === "response");
      assert.deepEqual(f.outcome, { outcome: "corrupt" });
    } else {
      assert.equal(y.socket.sent.length, 1);
      assert.equal(y.notes[0]?.note, "discarded");
    }
  }
});
Deno.test("provide_does_not_dispatch_queued_callback_after_close", async () => {
  const h = await open();
  h.socket.deliver(req(C, 1n));
  h.p.close();
  await tick();
  assert.equal(h.requests.length, 0);
  assert.equal(h.socket.sent.length, 1);
});
Deno.test("provide_has_no_client_liveness_timer", async () => {
  const original = globalThis.setTimeout;
  globalThis.setTimeout = (() => {
    throw Error("client timer installed");
  }) as unknown as typeof setTimeout;
  try {
    const h = await open();
    h.socket.deliver({ form: "subscribe", target: E });
    h.p.raise(1, 2, bytes, 0n);
    h.p.close();
  } finally {
    globalThis.setTimeout = original;
  }
});
Deno.test("provide_event_limits_read_and_error_outcomes", async () => {
  const h = await open();
  h.socket.deliver({ form: "subscribe", target: E });
  const n = h.socket.sent.length;
  assert.throws(() => h.p.raise(1, 2, new Uint8Array(3), 0n), SessionError);
  assert.equal(h.socket.sent.length, n);
  for (
    const outcome of [{ outcome: "corrupt" }, { outcome: "busy" }, {
      outcome: "contract",
      contract: { category: "invalid-value", rule: "range" },
    }] as const
  ) {
    const x = await open({ onRequest: () => outcome });
    x.socket.deliver(req(Q, 1n));
    await tick();
    const f = last(x.socket);
    assert(f.form === "response");
    assert.deepEqual(f.outcome, outcome);
  }
  h.socket.deliver({ form: "read", target: S });
  exact(h.socket, 2, sample(S, 0n, 77n, { provenance: "init" }));
  h.p.publish(1, 1, bytes, 0n); // read does not subscribe or send this value
  assert.equal(h.socket.sent.length, 3);
  h.socket.deliver({ form: "subscribe", target: S });
  h.p.invalidate(1, 1, 19n);
  h.socket.deliver({ form: "unsubscribe", target: S });
  h.socket.deliver({ form: "subscribe", target: S });
  exact(
    h.socket,
    h.socket.sent.length - 1,
    sample(S, 2n, 19n, { provenance: "invalid" }),
  );
});
Deno.test("provide_stream_forms_are_version_violations", async () => {
  for (const form of ["chunk", "stream_end", "stream_credit"] as const) {
    const h = await open();
    const base = { target: Q, correlation: 1n, stream: 0 };
    const f: Frame = form === "chunk"
      ? { form, ...base, envelope: { seq: 1n, stamp: 0n }, payload: bytes }
      : form === "stream_end"
      ? {
        form,
        ...base,
        envelope: { seq: 1n, stamp: 0n },
        outcome: { kind: "end" },
      }
      : { form, ...base, credit: 4 };
    h.socket.deliver(f);
    assert.deepEqual(h.socket.closes, [{}]);
    assert.deepEqual(h.notes, [{
      note: "violation",
      violation: "unknown-code",
    }]);
  }
});
Deno.test("provide_first_form_and_invalid_options", async () => {
  const h = start();
  const rejected = assert.rejects(h.pending, ProtocolError);
  h.socket.open("ridl-ws-v2-provider");
  h.socket.deliver({ form: "subscribe", target: S });
  await rejected;
  assert.deepEqual(h.socket.closes, [{}]);
  assert.deepEqual(h.notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
  assert.equal(h.socket.sent.length, 0);
  for (
    const extra of [
      { catalog: { name: "", hash: catalog.hash } },
      { catalog: { name: "alpha", hash: new Uint8Array(31) } },
      { members: [{ iface: 0, ord: 1, kind: "signal" }] },
      { members: [{ ...members[0], limit: -1 }] },
    ] as Partial<ProvideOptions>[]
  ) {
    let created = false;
    await assert.rejects(
      provide("ws://fixture", {
        catalog,
        encoding: "flatbuffers",
        members,
        onRequest: () => ({ outcome: "accepted" }),
        ...extra,
        webSocket: () => {
          created = true;
          throw Error("created");
        },
      }),
    );
    assert.equal(created, false);
  }
});
Deno.test("provide_fixed_metadata_is_not_provisioned", async () => {
  // The existing WebSocket e2e fixture supplies a real fixed ordinal.
  const catalog = await import("../e2e/catalog.ts");
  const target = { kind: Kind.Fixed, iface: catalog.IFACE, ord: catalog.FIXED };
  const h = start({
    catalog: { name: catalog.NAME, hash: catalog.HASH },
    members: [{ ...target, kind: "fixed" }],
    encoding: catalog.ENCODING,
  });
  h.socket.open("ridl-ws-v2-provider");
  attach(h, { name: catalog.NAME, hash: catalog.HASH });
  await h.pending;
  for (const form of ["read", "subscribe"] as const) {
    h.socket.deliver({ form, target });
    exact(h.socket, h.socket.sent.length - 1, {
      form: "answer",
      target,
      outcome: unknown,
    });
  }
});
Deno.test("provide_refusal_closed_reports_platform_result", async () => {
  const h = start();
  const error = h.pending.catch((e) => e);
  h.socket.open("ridl-ws-v2-provider");
  attach(h, { name: "missing" });
  const refusal = await error;
  h.socket.serverClose(1001, "peer left");
  assert.deepEqual(await refusal.closed, {
    code: 1001,
    reason: "peer left",
    refused: "unknown-catalog",
  });
});

Deno.test("provide_raised_limits_match_consumer_limits", async () => {
  const h = start({ members: [{ ...members[0], limit: 4096 }] });
  h.pending.catch(() => {});
  assert(h.socket, "a raised member limit must allow socket creation");
  h.socket.open("ridl-ws-v2-provider");
  attach(h);
  const p = await h.pending;
  h.socket.deliver({ form: "subscribe", target: S });
  const payload = new Uint8Array(4096);
  p.publish(1, 1, payload, 10n);
  exact(h.socket, 3, sample(S, 1n, 10n, { provenance: "live", payload }));
  assert.throws(
    () => p.publish(1, 1, new Uint8Array(4097), 11n),
    SessionError,
  );
});
Deno.test("provide_failed_clock_does_not_advance_response_sequence", async () => {
  let time = 77n;
  const h = await open({ clock: () => time });
  time = 1n << 63n;
  // Unknown targets settle synchronously inside the message handler.
  assert.doesNotThrow(() => h.socket.deliver(req(S, 1n)));
  assert.deepEqual(h.notes, [{
    note: "discarded",
    reason: "unexpected-outcome",
    frame: req(S, 1n),
  }]);
  assert.equal(h.socket.sent.length, 1);
  time = 88n;
  h.socket.deliver(req(S, 1n));
  exact(h.socket, 1, {
    form: "response",
    target: S,
    envelope: { seq: 1n, stamp: 88n },
    correlation: 1n,
    outcome: unknown,
  });
});

for (const mode of ["throw", "above-i64", "below-i64"] as const) {
  Deno.test(`provide_rejects_attach_clock_${mode}`, async () => {
    const failure = new Error("clock failed");
    let clocks = 0;
    const h = start({
      clock: () => {
        clocks++;
        if (mode === "throw") throw failure;
        return mode === "above-i64" ? 1n << 63n : -(1n << 63n) - 1n;
      },
    });
    let result: unknown = "pending";
    const done = h.pending.then(
      () => {
        result = "resolved";
      },
      (error) => {
        result = error;
      },
    );
    h.socket.open("ridl-ws-v2-provider");
    assert.doesNotThrow(() => attach(h));
    await tick();
    if (mode === "throw") assert.equal(result, failure);
    else {assert(
        result instanceof RangeError,
        "provide must reject an invalid attach stamp",
      );}
    await done;
    assert.deepEqual(h.socket.closes, [{}]);
    assert.equal(h.socket.sent.length, 0);
    // Closing need not produce a platform close event before sends stop.
    attach(h);
    h.socket.deliver({ form: "subscribe", target: S });
    h.socket.deliver(req(C, 1n));
    await tick();
    assert.equal(clocks, 1);
    assert.equal(h.socket.sent.length, 0);
    assert.equal(h.requests.length, 0);
  });
}
Deno.test("provide_rejects_unsafe_integer_limit_before_socket_creation", async () => {
  let created = false;
  await assert.rejects(
    provide("ws://fixture", {
      catalog,
      encoding: "flatbuffers",
      members: [{ ...members[0], limit: Number.MAX_SAFE_INTEGER + 1 }],
      onRequest: () => ({ outcome: "accepted" }),
      webSocket: () => {
        created = true;
        throw Error("socket created");
      },
    }),
    RangeError,
  );
  assert.equal(created, false);
});

Deno.test("provide_handler_failure_releases_each_capacity_slot", async () => {
  for (
    const fail of [() => {
      throw Error("sync");
    }, () => Promise.reject(Error("async"))]
  ) {
    let dispatched = 0;
    const h = await open({
      onRequest: () => {
        dispatched++;
        return fail();
      },
    });
    for (let seq = 1n; seq <= 65n; seq++) {
      h.socket.deliver(req(C, seq));
      await tick();
    }
    assert.equal(dispatched, 65);
    assert.equal(h.notes.length, 65);
    assert.equal(h.socket.sent.length, 1);
  }
});
Deno.test("provide_counters_are_independent_per_channel", async () => {
  const s2 = { ...S, iface: 2 }, e2 = { ...E, iface: 2 };
  const h = await open({
    members: [...members, { ...s2, kind: "signal" }, { ...e2, kind: "event" }],
  });
  for (const target of [S, s2, E, e2]) {
    h.socket.deliver({ form: "subscribe", target });
  }
  for (let seq = 1n; seq <= 2n; seq++) {
    for (const target of [S, s2, E, e2]) {
      if (target.kind === Kind.Signal) {
        h.p.publish(target.iface, target.ord, bytes, 1n);
      } else h.p.raise(target.iface, target.ord, bytes, 1n);
      const f = last(h.socket);
      assert(f.form === "publish" || f.form === "occurrence");
      assert.deepEqual(f.target, target);
      assert.equal(f.envelope.seq, seq);
    }
  }
});
Deno.test("provide_malformed_before_attach_is_unexpected_form", async () => {
  const h = start();
  const rejected = assert.rejects(h.pending, ProtocolError);
  h.socket.open("ridl-ws-v2-provider");
  const init = encode(sample(S, 0n, 0n, { provenance: "init" }));
  const malformed = new Uint8Array(init.length + 1);
  malformed.set(init);
  assert.deepEqual(decode(malformed), {
    ok: false,
    class: "discard",
    discard: "unexpected-payload",
  });
  h.socket.deliver(malformed);
  assert.deepEqual(h.socket.closes, [{}]);
  await rejected;
  assert.deepEqual(h.notes, [{
    note: "violation",
    violation: "unexpected-form",
  }]);
  assert.deepEqual(h.socket.closes, [{}]);
});
Deno.test("provide_violation_ends_operations_before_notifying", async () => {
  let observed = false;
  const h = await open({
    onNote: () => {
      try {
        h.p.publish(1, 1, bytes, 0n);
      } catch (error) {
        observed = error instanceof SessionError;
      }
    },
  });
  h.socket.deliver("text");
  assert(observed);
});
Deno.test("provide_prototype_member_kind_is_rejected", async () => {
  let created = false;
  await assert.rejects(
    provide("ws://fixture", {
      catalog,
      encoding: "flatbuffers",
      members: [
        {
          iface: 1,
          ord: 1,
          kind: "toString",
        } as unknown as ProvideOptions["members"][number],
      ],
      onRequest: () => ({ outcome: "accepted" }),
      webSocket: () => {
        created = true;
        throw Error("socket");
      },
    }),
    RangeError,
  );
  assert.equal(created, false);
});
Deno.test("provide_synchronous_settle_clock_failure_is_recorded_and_retryable", async () => {
  for (const mode of ["unknown", "corrupt", "busy"] as const) {
    let broken = false;
    const h = await open({
      clock: () => {
        if (broken) throw Error("clock");
        return 77n;
      },
      onRequest: () => new Promise(() => {}),
    });
    let seq = 1n;
    if (mode === "busy") {
      for (; seq <= 64n; seq++) h.socket.deliver(req(C, seq));
      await tick();
    }
    const frame = mode === "unknown"
      ? req(S, seq)
      : req(C, seq, mode === "corrupt" ? new Uint8Array(3) : bytes);
    broken = true;
    const sent = h.socket.sent.length;
    assert.doesNotThrow(() => h.socket.deliver(frame));
    assert.equal(h.socket.sent.length, sent);
    assert.deepEqual(h.notes, [{
      note: "discarded",
      reason: "unexpected-outcome",
      frame,
    }]);
    broken = false;
    h.socket.deliver(frame);
    const result = last(h.socket);
    assert(result.form === "response");
    assert.equal(result.correlation, seq);
    assert.equal(result.envelope.seq, 1n);
    assert.deepEqual(
      result.outcome,
      mode === "unknown"
        ? unknown
        : { outcome: mode === "busy" ? "busy" : "corrupt" },
    );
  }
});
