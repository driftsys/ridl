import assert from "node:assert/strict";
import { attach, AttachError, decode, encode, Kind } from "../mod.ts";
import type { Violation } from "../mod.ts";

const PUBLISH_LIVE = [
  0x07,
  0x01,
  1,
  0,
  0,
  0,
  2,
  0,
  0,
  0,
  0xab,
  0x89,
  0x67,
  0x45,
  0x23,
  0x01,
  0,
  0,
  7,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0x02,
  0xff,
  0x01,
];
const RESPONSE_ACCEPTED = [
  0x0a,
  0x03,
  1,
  0,
  0,
  0,
  3,
  0,
  0,
  0,
  0xac,
  0x89,
  0x67,
  0x45,
  0x23,
  0x01,
  0,
  0,
  1,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  1,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0x01,
  0x00,
];
const SUBSCRIBE = [0x03, 0x01, 1, 0, 0, 0, 2, 0, 0, 0];
const ANSWER = [0x04, 0x01, 1, 0, 0, 0, 2, 0, 0, 0, 0x01, 0x00];
const OCCURRENCE = [
  0x08,
  0x02,
  1,
  0,
  0,
  0,
  2,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  1,
  0,
  0,
  0,
  0,
  0,
  0,
  0,
  0xff,
];
const ATTACH = [
  0x01,
  1,
  0,
  0,
  0,
  1,
  ...new Array(32).fill(0),
  1,
  0x76,
];

function violation(message: number[]): Violation | string {
  const d = decode(new Uint8Array(message));
  if (d.ok) return "ok";
  return d.class === "violation" ? d.violation : d.discard;
}

function attachMessage(nameLen: number, name: number[]): number[] {
  return [0x01, 1, 0, 0, 0, 1, ...new Array(32).fill(0), nameLen, ...name];
}

function patched(base: number[], at: number, value: number): number[] {
  const copy = [...base];
  copy[at] = value;
  return copy;
}

Deno.test("an empty message is too short", () => {
  assert.equal(violation([]), "too-short");
});

Deno.test("a header cut short is too short", () => {
  assert.equal(violation(PUBLISH_LIVE.slice(0, 26)), "too-short");
});

Deno.test("a subscribe with trailing bytes is control too long", () => {
  assert.equal(violation([...SUBSCRIBE, 0]), "control-too-long");
});

Deno.test("form 0 and form 11 are unknown codes", () => {
  assert.equal(violation([0]), "unknown-code");
  assert.equal(violation([11]), "unknown-code");
});

Deno.test("kind 6 is an unknown code", () => {
  assert.equal(violation(patched(SUBSCRIBE, 1, 6)), "unknown-code");
});

Deno.test("provenance 4 is an unknown code", () => {
  assert.equal(violation(patched(PUBLISH_LIVE, 26, 4)), "unknown-code");
});

Deno.test("a refused response is an outcome not allowed", () => {
  const m = patched(patched(RESPONSE_ACCEPTED, 34, 9), 35, 2);
  assert.equal(violation(m), "outcome-not-allowed");
});

Deno.test("an accepted attached with detail 1 is nonzero detail", () => {
  assert.equal(violation([0x02, 0x01, 0x01]), "nonzero-detail");
});

Deno.test("interface 0 is a zero number", () => {
  assert.equal(violation(patched(SUBSCRIBE, 2, 0)), "zero-number");
});

Deno.test("ordinal 0 is a zero number", () => {
  assert.equal(violation(patched(SUBSCRIBE, 6, 0)), "zero-number");
});

Deno.test("name len 0 is a bad name length", () => {
  assert.equal(violation(attachMessage(0, [])), "bad-name-length");
});

Deno.test("name len longer than the message is a bad name length", () => {
  assert.equal(
    violation(attachMessage(9, [0x76, 0x65, 0x68])),
    "bad-name-length",
  );
});

Deno.test("a name that is not utf8 is a bad name", () => {
  assert.equal(violation(attachMessage(2, [0xff, 0xfe])), "bad-name");
});

Deno.test("an init publish with bytes after its header is discarded", () => {
  assert.equal(violation(patched(PUBLISH_LIVE, 26, 1)), "unexpected-payload");
});

Deno.test("an accepted response with bytes after its header is discarded", () => {
  assert.equal(violation([...RESPONSE_ACCEPTED, 0]), "unexpected-payload");
});

Deno.test("a live publish with an empty payload decodes", () => {
  const d = decode(new Uint8Array(PUBLISH_LIVE.slice(0, 27)));
  assert.ok(d.ok);
  assert.equal(d.frame.form, "publish");
  assert.deepEqual(d.frame.form === "publish" && d.frame.target, {
    kind: Kind.Signal,
    iface: 1,
    ord: 2,
  });
  assert.deepEqual(
    d.frame.form === "publish" && d.frame.state,
    { provenance: "live", payload: new Uint8Array(0) },
  );
});

Deno.test("attach refuses an empty name and a 256 byte name", () => {
  const hash = new Uint8Array(32);
  assert.throws(
    () => attach(1, 1, hash, ""),
    (e: unknown) => e instanceof AttachError && e.reason === "empty",
  );
  assert.throws(
    () => attach(1, 1, hash, "a".repeat(256)),
    (e: unknown) => e instanceof AttachError && e.reason === "too-long",
  );
  assert.doesNotThrow(() => attach(1, 1, hash, "a".repeat(255)));
});

Deno.test("a name of 85 three-byte characters is accepted and 86 is too long", () => {
  const hash = new Uint8Array(32);
  const ok = attach(1, 1, hash, "€".repeat(85));
  const bytes = encode(ok);
  assert.equal(bytes[38], 255);
  assert.equal(bytes.length, 39 + 255);
  const back = decode(bytes);
  assert.ok(back.ok);
  assert.deepEqual(back.frame, ok);
  assert.throws(
    () => attach(1, 1, hash, "€".repeat(86)),
    (e: unknown) => e instanceof AttachError && e.reason === "too-long",
  );
});

Deno.test("attach refuses an unknown encoding", () => {
  const hash = new Uint8Array(32);
  for (const tag of [0, 4]) {
    assert.throws(
      () => attach(1, tag, hash, "v"),
      (e: unknown) =>
        e instanceof AttachError && e.reason === "unknown-encoding",
    );
  }
  for (const tag of [1, 2, 3]) {
    assert.doesNotThrow(() => attach(1, tag, hash, "v"));
  }
});

Deno.test("attach refuses a hash that is not 32 bytes", () => {
  assert.throws(() => attach(1, 1, new Uint8Array(31), "v"), RangeError);
  assert.throws(() => attach(1, 1, new Uint8Array(33), "v"), RangeError);
});

Deno.test("an attach with encoding 0 or 4 is an unknown code", () => {
  for (const tag of [0, 4, 255]) {
    assert.equal(
      violation(patched(attachMessage(1, [0x76]), 5, tag)),
      "unknown-code",
    );
  }
});

Deno.test("an attach with bytes after its name is a bad name length", () => {
  assert.equal(violation(attachMessage(1, [0x76, 0x78])), "bad-name-length");
});

Deno.test("an attached with trailing bytes is control too long", () => {
  assert.equal(violation([0x02, 0x01, 0x00, 0x00]), "control-too-long");
});

Deno.test("an answer with trailing bytes is control too long", () => {
  assert.equal(violation([...ANSWER, 0]), "control-too-long");
});

Deno.test("a nonzero detail on a response is nonzero detail", () => {
  for (const outcome of [1, 2, 7, 8, 4, 5, 6]) {
    const m = patched(patched(RESPONSE_ACCEPTED, 34, outcome), 35, 1);
    assert.equal(violation(m), "nonzero-detail", `outcome ${outcome}`);
  }
});

Deno.test("a nonzero detail on an answer is nonzero detail", () => {
  for (const outcome of [1, 4, 5, 6]) {
    const m = patched(patched(ANSWER, 10, outcome), 11, 1);
    assert.equal(violation(m), "nonzero-detail", `outcome ${outcome}`);
  }
});

Deno.test("an answer outcome the form does not allow is an outcome not allowed", () => {
  for (const outcome of [2, 7, 8, 9]) {
    assert.equal(
      violation(patched(ANSWER, 10, outcome)),
      "outcome-not-allowed",
      `outcome ${outcome}`,
    );
  }
});

Deno.test("an unknown refusal reason is an unknown code", () => {
  for (const reason of [0, 6]) {
    assert.equal(violation([0x02, 0x09, reason]), "unknown-code");
  }
});

Deno.test("an unknown outcome code is an unknown code", () => {
  assert.equal(violation(patched(RESPONSE_ACCEPTED, 34, 10)), "unknown-code");
  assert.equal(violation(patched(RESPONSE_ACCEPTED, 34, 0)), "unknown-code");
  assert.equal(violation(patched(ANSWER, 10, 10)), "unknown-code");
  assert.equal(violation([0x02, 10, 0]), "unknown-code");
});

Deno.test("an unknown rule is an unknown code", () => {
  for (const rule of [0, 7]) {
    const m = patched(patched(RESPONSE_ACCEPTED, 34, 3), 35, rule);
    assert.equal(violation(m), "unknown-code");
  }
});

Deno.test("an invalid declared publish with bytes after its header is discarded", () => {
  assert.equal(violation(patched(PUBLISH_LIVE, 26, 3)), "unexpected-payload");
});

Deno.test("every form cut short of its header is too short", () => {
  const request = patched(OCCURRENCE, 0, 0x09);
  assert.equal(decode(new Uint8Array(ATTACH)).ok, true);
  for (let len = 1; len < 39; len++) {
    assert.equal(violation(ATTACH.slice(0, len)), "too-short", `attach ${len}`);
  }
  const whole = [
    [0x02, 0x01, 0x00],
    SUBSCRIBE,
    ANSWER,
    PUBLISH_LIVE.slice(0, 27),
    OCCURRENCE.slice(0, 26),
    request.slice(0, 26),
    RESPONSE_ACCEPTED,
  ];
  for (const m of whole) {
    assert.equal(decode(new Uint8Array(m)).ok, true, `form ${m[0]} whole`);
    for (let len = 1; len < m.length; len++) {
      assert.equal(
        violation(m.slice(0, len)),
        "too-short",
        `form ${m[0]} cut to ${len}`,
      );
    }
  }
});

Deno.test("decode returns payloads as views of the message", () => {
  const message = new Uint8Array(PUBLISH_LIVE);
  const d = decode(message);
  assert.ok(d.ok && d.frame.form === "publish");
  const state = d.frame.state;
  assert.ok(state.provenance === "live");
  assert.equal(state.payload.buffer, message.buffer);
});

Deno.test("an init or invalid publish with exactly one trailing byte is discarded", () => {
  const header = PUBLISH_LIVE.slice(0, 26);
  assert.equal(violation([...header, 1, 0]), "unexpected-payload");
  assert.equal(violation([...header, 3, 0]), "unexpected-payload");
  assert.equal(violation([...header, 1]), "ok");
  assert.equal(violation([...header, 3]), "ok");
});

Deno.test("an attach with name len 0 and encoding 0 is a bad name length", () => {
  assert.equal(
    violation(patched(attachMessage(0, []), 5, 0)),
    "bad-name-length",
  );
});

Deno.test("an attach with a bad name and encoding 0 is a bad name", () => {
  const m = patched(attachMessage(2, [0xff, 0xfe]), 5, 0);
  assert.equal(violation(m), "bad-name");
});

Deno.test("a response with interface 0 and outcome 10 is a zero number", () => {
  const m = patched(patched(RESPONSE_ACCEPTED, 2, 0), 34, 10);
  assert.equal(violation(m), "zero-number");
});

Deno.test("an answer with a trailing byte and interface 0 is control too long", () => {
  assert.equal(violation([...patched(ANSWER, 2, 0), 0]), "control-too-long");
});

Deno.test("a publish with interface 0 and provenance 4 is a zero number", () => {
  const m = patched(patched(PUBLISH_LIVE, 2, 0), 26, 4);
  assert.equal(violation(m), "zero-number");
});

Deno.test("a name that starts with a byte order mark decodes with it", () => {
  const d = decode(new Uint8Array(attachMessage(4, [0xef, 0xbb, 0xbf, 0x76])));
  assert.ok(d.ok && d.frame.form === "attach");
  assert.equal(d.frame.name, "\uFEFFv");
});
