import assert from "node:assert/strict";
import { decode, encode, Kind } from "../mod.ts";
import type { Frame, Target } from "../mod.ts";

function hex(text: string): Uint8Array {
  const digits = text.replace(/[`\s]/g, "");
  assert.equal(digits.length % 2, 0, `odd number of hex digits in ${text}`);
  const out = new Uint8Array(digits.length / 2);
  for (let i = 0; i < out.length; i++) {
    out[i] = parseInt(digits.slice(2 * i, 2 * i + 2), 16);
  }
  return out;
}

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
