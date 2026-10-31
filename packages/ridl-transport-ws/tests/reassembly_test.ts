import assert from "node:assert/strict";
import { BLOCK_BYTES, Kind } from "../mod.ts";
import type { Target } from "../mod.ts";
import { fixedLimit, Reassembly } from "../src/reassembly.ts";
import type { BlockFrame, BlockStep } from "../src/reassembly.ts";
import { pattern } from "./block_vectors.ts";

const FIXED: Target = { kind: Kind.Fixed, iface: 1, ord: 5 };
const LIMIT = 65_280;

/** The fields a test changes in a block after it is cut from its payload. */
interface Change {
  iface?: number;
  ord?: number;
  stamp?: bigint;
  seq?: bigint;
  total?: number;
}

/**
 * Block `index` of `payload`, whose `total` is `total`: interface 1, ordinal
 * 5, stamp 0x0123456789AB, seq 0, then the fields of `change`.
 */
function block(
  index: number,
  total: number,
  payload: Uint8Array,
  change: Change = {},
): BlockFrame {
  const start = BLOCK_BYTES * index;
  return {
    form: "block",
    target: {
      kind: Kind.Fixed,
      iface: change.iface ?? 1,
      ord: change.ord ?? 5,
    },
    envelope: { stamp: change.stamp ?? 0x0123456789ABn, seq: change.seq ?? 0n },
    total: change.total ?? total,
    index,
    payload: payload.subarray(start, Math.min(start + BLOCK_BYTES, total)),
  };
}

/** Every block of `payload`, in order. */
function blocks(payload: Uint8Array): BlockFrame[] {
  const count = Math.ceil(payload.length / BLOCK_BYTES);
  return Array.from(
    { length: count },
    (_, i) => block(i, payload.length, payload),
  );
}

/** Applies `frames` in order and returns each step. */
function acceptAll(
  r: Reassembly,
  frames: BlockFrame[],
  limit = LIMIT,
): BlockStep[] {
  return frames.map((b) => r.accept(b, limit));
}

const steps = (list: BlockStep[]) => list.map((s) => s.step);

function completed(step: BlockStep): Uint8Array {
  assert.equal(step.step, "complete");
  return step.step === "complete" ? step.payload : new Uint8Array();
}

Deno.test("the blocks of a payload complete in order, each at 4,096 times its index", () => {
  const r = new Reassembly();
  const data = pattern(8193);
  const result = acceptAll(r, blocks(data));
  assert.deepEqual(steps(result), ["collecting", "collecting", "complete"]);
  assert.deepEqual(completed(result[2]), data);
  assert.equal(r.holds(FIXED), false);
});

Deno.test("a payload of one block completes at once", () => {
  for (const total of [1, 4096]) {
    const r = new Reassembly();
    const data = pattern(total);
    const step = r.accept(block(0, total, data), LIMIT);
    assert.deepEqual(completed(step), data);
    assert.equal(r.holds(FIXED), false);
  }
});

Deno.test("an exact multiple of 4,096 completes after its last full block", () => {
  const r = new Reassembly();
  const data = pattern(32_768);
  const result = acceptAll(r, blocks(data));
  assert.equal(result.length, 8);
  assert.deepEqual(steps(result), [
    ...Array(7).fill("collecting"),
    "complete",
  ]);
  assert.deepEqual(completed(result[7]), data);
  // No ninth block is expected: an index 0 starts the next payload.
  assert.equal(r.holds(FIXED), false);
  assert.equal(r.accept(block(0, 1, pattern(1)), LIMIT).step, "complete");
});

Deno.test("a total equal to the limit is collected and one byte more is over-limit", () => {
  const r = new Reassembly();
  const equal = pattern(8192);
  const result = acceptAll(r, blocks(equal), 8192);
  assert.deepEqual(steps(result), ["collecting", "complete"]);
  assert.deepEqual(completed(result[1]), equal);

  const above = blocks(pattern(8193));
  assert.equal(r.accept(above[0], 8192).step, "over-limit");
  assert.equal(r.holds(FIXED), true);
  assert.equal(r.accept(above[1], 8192).step, "skipping");
  assert.equal(r.holds(FIXED), true);
  assert.equal(r.accept(above[2], 8192).step, "skipping");
  assert.equal(r.holds(FIXED), false);

  // One block above the limit: nothing is left to skip.
  const one = new Reassembly();
  assert.equal(one.accept(block(0, 101, pattern(101)), 100).step, "over-limit");
  assert.equal(one.holds(FIXED), false);
  const next = one.accept(block(0, 100, pattern(100)), 100);
  assert.deepEqual(completed(next), pattern(100));
});

Deno.test("a limit above the fixed ceiling, or not a finite number, is held", () => {
  assert.equal(fixedLimit(2 ** 32 - 1), 65_280);
  assert.equal(fixedLimit(65_281), 65_280);
  assert.equal(fixedLimit(65_280), 65_280);
  assert.equal(fixedLimit(20_000), 20_000);
  assert.equal(fixedLimit(NaN), 0);
  assert.equal(fixedLimit(Infinity), 0);
});

Deno.test("a completed payload keeps its bytes after the next payload completes", () => {
  const r = new Reassembly();
  const first = acceptAll(r, blocks(pattern(8193)));
  const kept = completed(first[2]);
  const second = acceptAll(r, blocks(new Uint8Array(8193).fill(0xff)));
  assert.deepEqual(completed(second[2]), new Uint8Array(8193).fill(0xff));
  assert.deepEqual(kept, pattern(8193));
});

Deno.test("holds compares the kind as well", () => {
  const r = new Reassembly();
  assert.equal(
    r.accept(block(0, 8193, pattern(8193)), LIMIT).step,
    "collecting",
  );
  assert.equal(r.holds(FIXED), true);
  assert.equal(r.holds({ kind: Kind.Signal, iface: 1, ord: 5 }), false);
  assert.equal(r.holds({ kind: Kind.Fixed, iface: 2, ord: 5 }), false);
  assert.equal(r.holds({ kind: Kind.Fixed, iface: 1, ord: 6 }), false);
});

Deno.test("a missing, a repeated and a reordered block are sequence errors", () => {
  const all = blocks(pattern(8193));
  const cases: [number[], string[]][] = [
    [[0, 2], ["collecting", "sequence"]],
    [[0, 1, 1], ["collecting", "collecting", "sequence"]],
    [[0, 2, 1], ["collecting", "sequence", "sequence"]],
  ];
  for (const [order, expected] of cases) {
    const r = new Reassembly();
    assert.deepEqual(steps(acceptAll(r, order.map((i) => all[i]))), expected);
    assert.equal(r.holds(FIXED), false);
  }
});

Deno.test("an index above 0 with nothing in progress is a sequence error", () => {
  const r = new Reassembly();
  assert.equal(r.accept(block(0, 1, pattern(1)), LIMIT).step, "complete");
  const step = r.accept(block(1, 8193, pattern(8193)), LIMIT);
  assert.equal(step.step, "sequence");
  assert.equal(r.holds(FIXED), false);
});

Deno.test("an index 0 while a payload is in progress is a sequence error, for the same identity too", () => {
  for (const second of [{ ord: 6 }, {}]) {
    const r = new Reassembly();
    assert.equal(
      r.accept(block(0, 8193, pattern(8193)), LIMIT).step,
      "collecting",
    );
    const step = r.accept(block(0, 8193, pattern(8193), second), LIMIT);
    assert.equal(step.step, "sequence");
    assert.equal(r.holds(FIXED), false);
  }
});

Deno.test("a block whose interface, ordinal, stamp, seq or total differs is a sequence error", () => {
  const changes: Change[] = [
    { iface: 2 },
    { ord: 6 },
    { stamp: 0x0123456789ACn },
    { seq: 1n },
    { total: 8194 },
  ];
  for (const change of changes) {
    const r = new Reassembly();
    const data = pattern(8193);
    assert.equal(r.accept(block(0, 8193, data), LIMIT).step, "collecting");
    const step = r.accept(block(1, 8193, data, change), LIMIT);
    assert.equal(step.step, "sequence", JSON.stringify(change, String));
    assert.equal(r.holds(FIXED), false);
  }
});

Deno.test("the sequence of a skipped payload is still checked", () => {
  const data = pattern(8193);
  const r = new Reassembly();
  assert.equal(r.accept(block(0, 8193, data), 8192).step, "over-limit");
  assert.equal(r.accept(block(2, 8193, data), 8192).step, "sequence");
  assert.equal(r.holds(FIXED), false);
  const changes: Change[] = [{ ord: 6 }, { seq: 1n }, { total: 8194 }];
  for (const change of changes) {
    const fresh = new Reassembly();
    assert.equal(fresh.accept(block(0, 8193, data), 8192).step, "over-limit");
    const step = fresh.accept(block(1, 8193, data, change), 8192);
    assert.equal(step.step, "sequence", JSON.stringify(change, String));
    assert.equal(fresh.holds(FIXED), false);
  }
});

Deno.test("after a payload completes, the next index 0 starts a new payload", () => {
  const r = new Reassembly();
  assert.equal(steps(acceptAll(r, blocks(pattern(5000)))).at(-1), "complete");
  const other = pattern(4097).map((b) => b ^ 0x55);
  const result = acceptAll(r, blocks(other));
  assert.deepEqual(steps(result), ["collecting", "complete"]);
  assert.deepEqual(completed(result[1]), other);
});

Deno.test("clear drops the payload in progress", () => {
  const r = new Reassembly();
  const all = blocks(pattern(8193));
  assert.equal(r.accept(all[0], LIMIT).step, "collecting");
  r.clear();
  assert.equal(r.holds(FIXED), false);
  assert.equal(r.accept(all[1], LIMIT).step, "sequence");
  assert.equal(r.accept(all[0], LIMIT).step, "collecting");
});
