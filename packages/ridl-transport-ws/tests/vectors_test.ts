import assert from "node:assert/strict";
import { decode, encode, Kind } from "../mod.ts";
import type { Frame, Target } from "../mod.ts";
import {
  BLOCK_LAST,
  BLOCK_ONE,
  blockHeader,
  blockMessage,
  concat,
  hex,
  pattern,
  sha256,
} from "./block_vectors.ts";

function target(kind: Kind, iface: number, ord: number): Target {
  return { kind, iface, ord };
}

const KNOWN = [
  "attach-1",
  "attached-accepted",
  "attached-mismatch",
  "subscribe-signal",
  "answer-accepted",
  "publish-live",
  "request-command",
  "response-accepted",
  "response-violation",
];

function expected(id: string): Frame | undefined {
  const hash = new Uint8Array(32);
  for (let i = 0; i < 32; i++) hash[i] = i;
  switch (id) {
    case "attach-1":
      return {
        form: "attach",
        frameVersion: 1,
        encoding: 1,
        hash,
        name: "veh.cabin",
      };
    case "attached-accepted":
      return { form: "attached", outcome: { outcome: "accepted" } };
    case "attached-mismatch":
      return {
        form: "attached",
        outcome: { outcome: "refused", reason: "catalog-mismatch" },
      };
    case "subscribe-signal":
      return { form: "subscribe", target: target(Kind.Signal, 1, 2) };
    case "answer-accepted":
      return {
        form: "answer",
        target: target(Kind.Signal, 1, 2),
        outcome: { outcome: "accepted" },
      };
    case "publish-live":
      return {
        form: "publish",
        target: target(Kind.Signal, 1, 2),
        envelope: { stamp: 0x0123456789ABn, seq: 7n },
        state: { provenance: "live", payload: new Uint8Array([0xff, 0x01]) },
      };
    case "request-command":
      return {
        form: "request",
        target: target(Kind.Command, 1, 3),
        envelope: { stamp: 0x0123456789ABn, seq: 1n },
        payload: new Uint8Array([0x01]),
      };
    case "response-accepted":
      return {
        form: "response",
        target: target(Kind.Command, 1, 3),
        envelope: { stamp: 0x0123456789ACn, seq: 1n },
        correlation: 1n,
        outcome: { outcome: "accepted" },
      };
    case "response-violation":
      return {
        form: "response",
        target: target(Kind.Query, 1, 4),
        envelope: { stamp: 0x0123456789ACn, seq: 2n },
        correlation: 2n,
        outcome: {
          outcome: "contract",
          contract: { category: "invalid-value", rule: "range" },
        },
      };
    default:
      return undefined;
  }
}

Deno.test("every spec vector decodes and re-encodes byte for byte", async () => {
  const text = await Deno.readTextFile(
    new URL(
      "../../../docs/specification/frame-specification.md",
      import.meta.url,
    ),
  );
  const section = text.slice(text.indexOf("### 11.1"));
  const lines = section.split("\n");
  const header = lines.findIndex((l) =>
    /^\|\s*Id\s*\|\s*Vector\s*\|\s*Hex\s*\|/.test(l)
  );
  assert.ok(header >= 0, "no vector table found under 11.1");
  const rows: string[] = [];
  for (let i = header + 2; i < lines.length && lines[i].startsWith("|"); i++) {
    rows.push(lines[i]);
  }
  assert.ok(rows.length >= 9, `only ${rows.length} vector rows found`);
  const seen: string[] = [];
  for (const row of rows) {
    const cells = row.split("|").map((c) => c.trim());
    const id = cells[1].replace(/`/g, "");
    seen.push(id);
    const bytes = hex(cells[3]);
    const want = expected(id);
    assert.ok(want, `vector ${id} is not known to this test`);
    const got = decode(bytes);
    assert.ok(got.ok, `vector ${id} did not decode`);
    assert.deepEqual(got.frame, want, `vector ${id}`);
    assert.deepEqual(encode(want), bytes, `encoding of ${id}`);
  }
  assert.deepEqual([...seen].sort(), [...KNOWN].sort());
});

const FIXED_1_5 = { kind: Kind.Fixed, iface: 1, ord: 5 };
const BLOCK_ENVELOPE = { stamp: 0x0123456789ABn, seq: 0n };

function block(total: number, index: number, payload: Uint8Array): Frame {
  return {
    form: "block",
    target: FIXED_1_5,
    envelope: BLOCK_ENVELOPE,
    total,
    index,
    payload,
  };
}

/**
 * The bytes of one row of the version 2 vector table of the frame
 * specification, by its `Id`.
 */
async function specVersion2Vector(id: string): Promise<Uint8Array> {
  const text = await Deno.readTextFile(
    new URL(
      "../../../docs/specification/frame-specification.md",
      import.meta.url,
    ),
  );
  const start = text.indexOf("**Version 2 test vectors.**");
  assert.ok(start >= 0, "no version 2 vector paragraph");
  for (const line of text.slice(start).split("\n")) {
    const cells = line.split("|").map((c) => c.trim());
    if (line.startsWith("|") && cells[1] === `\`${id}\``) return hex(cells[3]);
  }
  throw new Error(`no version 2 vector ${id} in the specification`);
}

Deno.test("every version 2 vector decodes and re-encodes byte for byte", async () => {
  const hash = new Uint8Array(32);
  for (let i = 0; i < 32; i++) hash[i] = i;
  const vectors: [string, Uint8Array, Frame][] = [
    [
      "attach-2",
      await specVersion2Vector("attach-2"),
      {
        form: "attach",
        frameVersion: 2,
        encoding: 1,
        hash,
        name: "veh.cabin",
      },
    ],
    [
      "block-last",
      await specVersion2Vector("block-last"),
      block(4098, 1, Uint8Array.of(0xab, 0xcd)),
    ],
    [
      "block-one",
      await specVersion2Vector("block-one"),
      block(1, 0, Uint8Array.of(0xab)),
    ],
    [
      "block-first",
      blockMessage(4098, 0, pattern(4096)),
      block(4098, 0, pattern(4096)),
    ],
    [
      "block-full",
      blockMessage(4096, 0, new Uint8Array(4096)),
      block(4096, 0, new Uint8Array(4096)),
    ],
    [
      "block-max-last",
      blockMessage(0xffffffff, 0xfffff, new Uint8Array(4095)),
      block(0xffffffff, 0xfffff, new Uint8Array(4095)),
    ],
  ];
  assert.equal(vectors[3][1].length, 4130);
  assert.equal(vectors[5][1].length, 4129);
  for (const [id, bytes, want] of vectors) {
    const got = decode(bytes);
    assert.ok(got.ok, `vector ${id} did not decode`);
    assert.deepEqual(got.frame, want, `vector ${id}`);
    assert.deepEqual(encode(want), bytes, `encoding of ${id}`);
  }
});

Deno.test("the shared block-last and block-one constants are the specification's bytes", async () => {
  assert.deepEqual(BLOCK_LAST, await specVersion2Vector("block-last"));
  assert.deepEqual(BLOCK_ONE, await specVersion2Vector("block-one"));
});

Deno.test("the first block of a 4,098-byte payload carries 4,096 bytes of the pattern", async () => {
  const first = decode(blockMessage(4098, 0, pattern(4096)));
  assert.ok(first.ok && first.frame.form === "block");
  const last = decode(BLOCK_LAST);
  assert.ok(last.ok && last.frame.form === "block");
  assert.deepEqual(first.frame.payload, pattern(4096));
  assert.equal(
    await sha256(concat(first.frame.payload, last.frame.payload)),
    "8830a559eb9dfd3795c81b96aaa42875e12ce7fb6348fccf47757b55cfefc7fe",
  );
});

Deno.test("a last block of the largest total decodes, and with 4,096 bytes is a bad block", () => {
  const ok = decode(blockMessage(0xffffffff, 0xfffff, new Uint8Array(4095)));
  assert.ok(ok.ok && ok.frame.form === "block");
  assert.equal(ok.frame.total, 4_294_967_295);
  assert.equal(ok.frame.index, 1_048_575);
  assert.equal(ok.frame.payload.length, 4095);
  assert.ok(decode(blockMessage(4096, 0, new Uint8Array(4096))).ok);
  const bad = decode(blockMessage(0xffffffff, 0xfffff, new Uint8Array(4096)));
  assert.deepEqual(bad, {
    ok: false,
    class: "violation",
    violation: "bad-block",
  });
});

Deno.test("the 16 blocks of a 65,280-byte payload encode to the shared sequence", async () => {
  const payload = pattern(65_280);
  const messages: Uint8Array[] = [];
  for (let k = 0; k < 16; k++) {
    const part = payload.subarray(4096 * k, Math.min(4096 * (k + 1), 65_280));
    messages.push(encode(block(65_280, k, part)));
  }
  assert.deepEqual(
    messages.map((m) => m.length),
    [...new Array(15).fill(4130), 3874],
  );
  assert.deepEqual(
    messages[0].subarray(0, 34),
    hex(blockHeader(65_280, 0)),
  );
  assert.equal(
    [...messages[0].subarray(0, 34)].map((b) => b.toString(16).padStart(2, "0"))
      .join(""),
    "0b0501000000050000" + "00ab89674523010000" + "0000000000000000" +
      "00ff0000" + "00000000",
  );
  assert.deepEqual(
    messages[15].subarray(26, 34),
    hex("00ff0000 0f000000"),
  );
  assert.deepEqual(
    messages[15].subarray(34, 38),
    Uint8Array.of(0xc4, 0xc5, 0xc6, 0xc7),
  );
  assert.equal(
    await sha256(concat(...messages)),
    "4738f9ca3db820c9597bcd0785a9b194a2b21eb0096f31cd94641b6934c26a58",
  );
  assert.equal(
    await sha256(payload),
    "218da22426f9259c2bd8bad291c6f70c5ca96a5f402c503f5700e2f7ba2a0ba7",
  );
});
