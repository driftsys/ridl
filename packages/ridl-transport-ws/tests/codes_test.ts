import assert from "node:assert/strict";
import { decode, encode, encodingTag, Kind } from "../mod.ts";
import type { Contract, Frame, Reason, Rule } from "../mod.ts";

/** Little-endian bytes of a u32. */
function u32(v: number): number[] {
  return [v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, (v >>> 24) & 0xff];
}

/** Little-endian bytes of a 64-bit value. */
function u64(v: bigint): number[] {
  const out: number[] = [];
  for (let i = 0n; i < 8n; i++) out.push(Number((v >> (8n * i)) & 0xffn));
  return out;
}

const T = (kind: number) => [kind, ...u32(1), ...u32(2)];
const E = [...u64(0x0123456789ABn), ...u64(7n)];
const target = (kind: Kind) => ({ kind, iface: 1, ord: 2 });
const envelope = { stamp: 0x0123456789ABn, seq: 7n };

function roundTrip(bytes: number[], frame: Frame) {
  const message = new Uint8Array(bytes);
  assert.deepEqual(decode(message), { ok: true, frame });
  assert.deepEqual(encode(frame), message);
}

Deno.test("each control form code maps to its form", () => {
  roundTrip([3, ...T(1)], { form: "subscribe", target: target(Kind.Signal) });
  roundTrip([5, ...T(1)], {
    form: "unsubscribe",
    target: target(Kind.Signal),
  });
  roundTrip([6, ...T(1)], { form: "read", target: target(Kind.Signal) });
  roundTrip([2, 1, 0], { form: "attached", outcome: { outcome: "accepted" } });
  roundTrip([4, ...T(1), 1, 0], {
    form: "answer",
    target: target(Kind.Signal),
    outcome: { outcome: "accepted" },
  });
  const hash = new Uint8Array(32).fill(9);
  roundTrip([1, ...u32(1), 2, ...hash, 1, 0x61], {
    form: "attach",
    frameVersion: 1,
    encoding: 2,
    hash,
    name: "a",
  });
});

Deno.test("each data form code maps to its form", () => {
  roundTrip([7, ...T(1), ...E, 1], {
    form: "publish",
    target: target(Kind.Signal),
    envelope,
    state: { provenance: "init" },
  });
  roundTrip([8, ...T(2), ...E, 5], {
    form: "occurrence",
    target: target(Kind.Event),
    envelope,
    payload: new Uint8Array([5]),
  });
  roundTrip([9, ...T(3), ...E, 5], {
    form: "request",
    target: target(Kind.Command),
    envelope,
    payload: new Uint8Array([5]),
  });
  roundTrip([10, ...T(3), ...E, ...u64(7n), 1, 0], {
    form: "response",
    target: target(Kind.Command),
    envelope,
    correlation: 7n,
    outcome: { outcome: "accepted" },
  });
  roundTrip([11, ...T(5), ...E, ...u32(1), ...u32(0), 0xab], {
    form: "block",
    target: target(Kind.Fixed),
    envelope,
    total: 1,
    index: 0,
    payload: new Uint8Array([0xab]),
  });
});

Deno.test("each kind code maps to its kind", () => {
  const kinds: [number, Kind][] = [
    [1, Kind.Signal],
    [2, Kind.Event],
    [3, Kind.Command],
    [4, Kind.Query],
    [5, Kind.Fixed],
  ];
  for (const [code, kind] of kinds) {
    assert.equal(kind, code);
    roundTrip([3, ...T(code)], { form: "subscribe", target: target(kind) });
  }
});

Deno.test("each provenance code maps to its state", () => {
  roundTrip([7, ...T(1), ...E, 1], {
    form: "publish",
    target: target(Kind.Signal),
    envelope,
    state: { provenance: "init" },
  });
  roundTrip([7, ...T(1), ...E, 2, 0xaa], {
    form: "publish",
    target: target(Kind.Signal),
    envelope,
    state: { provenance: "live", payload: new Uint8Array([0xaa]) },
  });
  roundTrip([7, ...T(1), ...E, 3], {
    form: "publish",
    target: target(Kind.Signal),
    envelope,
    state: { provenance: "invalid" },
  });
});

const RULES: [number, Rule][] = [
  [1, "range"],
  [2, "step"],
  [3, "length"],
  [4, "pattern"],
  [5, "variant"],
  [6, "unique"],
];

Deno.test("each response outcome code maps to its outcome", () => {
  const head = [10, ...T(4), ...E, ...u64(7n)];
  const tgt = target(Kind.Query);
  const make = (outcome: unknown): Frame =>
    ({
      form: "response",
      target: tgt,
      envelope,
      correlation: 7n,
      outcome,
    }) as Frame;
  roundTrip([...head, 1, 0], make({ outcome: "accepted" }));
  roundTrip(
    [...head, 2, 0, 1, 2],
    make({ outcome: "reply", payload: new Uint8Array([1, 2]) }),
  );
  for (const [code, rule] of RULES) {
    roundTrip(
      [...head, 3, code],
      make({
        outcome: "contract",
        contract: { category: "invalid-value", rule },
      }),
    );
  }
  const others: [number, Contract][] = [
    [4, { category: "precondition-failed" }],
    [5, { category: "contract-broken" }],
    [6, { category: "unknown-interaction" }],
  ];
  for (const [code, contract] of others) {
    roundTrip([...head, code, 0], make({ outcome: "contract", contract }));
  }
  roundTrip([...head, 7, 0], make({ outcome: "corrupt" }));
  roundTrip([...head, 8, 0], make({ outcome: "busy" }));
});

Deno.test("each answer outcome code maps to its outcome", () => {
  const head = [4, ...T(1)];
  const make = (outcome: unknown): Frame =>
    ({ form: "answer", target: target(Kind.Signal), outcome }) as Frame;
  roundTrip([...head, 1, 0], make({ outcome: "accepted" }));
  for (const [code, rule] of RULES) {
    roundTrip(
      [...head, 3, code],
      make({
        outcome: "contract",
        contract: { category: "invalid-value", rule },
      }),
    );
  }
  const others: [number, Contract][] = [
    [4, { category: "precondition-failed" }],
    [5, { category: "contract-broken" }],
    [6, { category: "unknown-interaction" }],
  ];
  for (const [code, contract] of others) {
    roundTrip([...head, code, 0], make({ outcome: "contract", contract }));
  }
});

Deno.test("each attached outcome and refusal reason maps to its code", () => {
  roundTrip([2, 1, 0], { form: "attached", outcome: { outcome: "accepted" } });
  const reasons: [number, Reason][] = [
    [1, "unknown-catalog"],
    [2, "catalog-mismatch"],
    [3, "encoding-unsupported"],
    [4, "frame-version-unsupported"],
    [5, "range"],
  ];
  for (const [code, reason] of reasons) {
    roundTrip([2, 9, code], {
      form: "attached",
      outcome: { outcome: "refused", reason },
    });
  }
});

Deno.test("each encoding tag maps to its name on attach", () => {
  assert.equal(encodingTag("flatbuffers"), 1);
  assert.equal(encodingTag("proto3"), 2);
  assert.equal(encodingTag("podbuf"), 3);
  const hash = new Uint8Array(32);
  for (const tag of [1, 2, 3]) {
    roundTrip([1, ...u32(1), tag, ...hash, 1, 0x76], {
      form: "attach",
      frameVersion: 1,
      encoding: tag,
      hash,
      name: "v",
    });
  }
});

Deno.test("a zero-length payload is an empty Uint8Array, not null", () => {
  const empty = new Uint8Array(0);
  const cases: [number[], Frame][] = [
    [[9, ...T(3), ...E], {
      form: "request",
      target: target(Kind.Command),
      envelope,
      payload: empty,
    }],
    [[8, ...T(2), ...E], {
      form: "occurrence",
      target: target(Kind.Event),
      envelope,
      payload: empty,
    }],
    [[7, ...T(1), ...E, 2], {
      form: "publish",
      target: target(Kind.Signal),
      envelope,
      state: { provenance: "live", payload: empty },
    }],
    [[10, ...T(4), ...E, ...u64(7n), 2, 0], {
      form: "response",
      target: target(Kind.Query),
      envelope,
      correlation: 7n,
      outcome: { outcome: "reply", payload: empty },
    }],
  ];
  for (const [bytes, frame] of cases) {
    roundTrip(bytes, frame);
    const decoded = decode(new Uint8Array(bytes));
    assert.ok(decoded.ok);
    const f = decoded.frame;
    const payload = f.form === "publish"
      ? (f.state as { payload: Uint8Array }).payload
      : f.form === "response"
      ? (f.outcome as { payload: Uint8Array }).payload
      : (f as { payload: Uint8Array }).payload;
    assert.ok(payload instanceof Uint8Array);
    assert.equal(payload.length, 0);
  }
});

Deno.test("extreme integers keep their signedness on the wire", () => {
  const stamp = -1n;
  const seq = 0xFFFFFFFFFFFFFFFFn;
  const correlation = 1n << 63n;
  const iface = 0xFFFFFFFF;
  const ord = 0x80000000;
  const t = (kind: number) => [kind, ...u32(iface), ...u32(ord)];
  const tgt = (kind: Kind) => ({ kind, iface, ord });
  const env = { stamp, seq };
  const e = [...u64(stamp), ...u64(seq)];
  roundTrip([7, ...t(1), ...e, 2, 9], {
    form: "publish",
    target: tgt(Kind.Signal),
    envelope: env,
    state: { provenance: "live", payload: new Uint8Array([9]) },
  });
  roundTrip([8, ...t(2), ...e, 9], {
    form: "occurrence",
    target: tgt(Kind.Event),
    envelope: env,
    payload: new Uint8Array([9]),
  });
  roundTrip([9, ...t(3), ...e, 9], {
    form: "request",
    target: tgt(Kind.Command),
    envelope: env,
    payload: new Uint8Array([9]),
  });
  roundTrip([10, ...t(4), ...e, ...u64(correlation), 1, 0], {
    form: "response",
    target: tgt(Kind.Query),
    envelope: env,
    correlation,
    outcome: { outcome: "accepted" },
  });
});

Deno.test("a message at a nonzero byte offset decodes to the same frame", () => {
  const bytes = [9, ...T(3), ...E, 5];
  const backing = new Uint8Array(bytes.length + 3);
  backing.set(bytes, 3);
  const view = backing.subarray(3);
  assert.deepEqual(decode(view), {
    ok: true,
    frame: {
      form: "request",
      target: target(Kind.Command),
      envelope,
      payload: new Uint8Array([5]),
    },
  });
});

Deno.test("data payloads are views of the message", () => {
  const cases: number[][] = [
    [9, ...T(3), ...E, 5],
    [8, ...T(2), ...E, 5],
    [10, ...T(4), ...E, ...u64(7n), 2, 0, 5],
    [11, ...T(5), ...E, ...u32(1), ...u32(0), 5],
  ];
  for (const bytes of cases) {
    const message = new Uint8Array(bytes);
    const d = decode(message);
    assert.ok(d.ok);
    const f = d.frame;
    const payload = f.form === "response"
      ? (f.outcome as { payload: Uint8Array }).payload
      : (f as { payload: Uint8Array }).payload;
    assert.equal(payload.buffer, message.buffer);
    assert.equal(payload.length, 1);
  }
});

Deno.test("an encoded attach with a bad name length throws a RangeError", () => {
  const hash = new Uint8Array(32);
  const frame = (name: string): Frame => ({
    form: "attach",
    frameVersion: 1,
    encoding: 1,
    hash,
    name,
  });
  assert.throws(() => encode(frame("")), RangeError);
  assert.throws(() => encode(frame("a".repeat(256))), RangeError);
  assert.equal(encode(frame("a".repeat(255))).length, 39 + 255);
});

Deno.test("a name that starts with a byte order mark keeps it", () => {
  const hash = new Uint8Array(32);
  roundTrip([1, ...u32(1), 1, ...hash, 4, 0xef, 0xbb, 0xbf, 0x76], {
    form: "attach",
    frameVersion: 1,
    encoding: 1,
    hash,
    name: "\uFEFFv",
  });
});
