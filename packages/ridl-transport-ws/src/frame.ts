/**
 * The header codec of the WebSocket binding (frame specification §11.1).
 *
 * One frame is one WebSocket message. {@link decode} reads one message without
 * any session context and {@link encode} writes one. Every integer is
 * fixed-width little-endian, a payload is every byte after the header, and
 * whether a frame carries a payload is decided from its header alone.
 *
 * The `block` form (form code 11) belongs to binding version 2
 * (frame specification §11.1, binding version 2): one part of the payload of
 * a `publish` of a `fixed` that does not fit one message. Every block but the
 * last carries {@link BLOCK_BYTES} bytes. The codec reads and writes the form
 * on any session; whether a session accepts it is the session's to decide.
 *
 * The stream forms `chunk`, `stream_end` and `stream_credit` (form codes 12,
 * 13 and 14) belong to binding version 3 (frame specification §11.1, binding
 * version 3). The decoder checks each by itself: the `kind` of a stream frame
 * is `Command` or `Query`, a `stream_end` has one of the outcomes of
 * {@link StreamOutcome}, and a `stream_credit` grants at least one chunk. The
 * codec reads and writes the forms on any session; whether a session accepts
 * them is the session's to decide.
 *
 * @module
 */

import { BLOCK_BYTES, blockCount, FORM_BLOCK } from "./blocks.ts";

/**
 * The newest frame version without streams, 2, which this binding carries
 * with {@link FRAME_VERSION_3}, the frame version that adds the stream forms.
 */
export const FRAME_VERSION = 2;
/** The WebSocket subprotocol name of binding version 2. */
export const SUBPROTOCOL = "ridl-ws-v2";
/** The WebSocket subprotocol with a version 2 provider client. */
export const SUBPROTOCOL_PROVIDER = "ridl-ws-v2-provider";
/** The frame version that adds the stream forms. */
export const FRAME_VERSION_3 = 3;
/** The WebSocket subprotocol name of binding version 3. */
export const SUBPROTOCOL_V3 = "ridl-ws-v3";
/**
 * The number of chunks the receiver of a stream grants the sender before it
 * sends any `stream_credit` (frame specification §11.1, binding version 3).
 */
export const INITIAL_CREDIT = 4;
/**
 * The subprotocol name of binding version 1, which a server of frame version
 * 1 selects.
 */
export const SUBPROTOCOL_V1 = "ridl-ws-v1";
/**
 * The smallest message size limit a receiver may configure: the largest header
 * of any message, an `attach` with a 255-byte name.
 */
export const MIN_MESSAGE_LIMIT = 294;
/** The size of an encoded {@link Envelope} in bytes. */
export const ENVELOPE_BYTES = 16;
export { BLOCK_BYTES };

/** The interaction kinds and their wire codes. */
export const Kind = {
  Signal: 1,
  Event: 2,
  Command: 3,
  Query: 4,
  Fixed: 5,
} as const;
/** The wire code of an interaction kind. */
export type Kind = (typeof Kind)[keyof typeof Kind];

/**
 * The default ceiling of a `signal` and an `event`, in bytes of encoded
 * payload: one 4,096-byte page minus 256 bytes of header. The limit of a
 * member that no `[limits]` entry of the catalog (`[limits.interaction]`,
 * `[limits.kind]` or `default`) gives one. The
 * Rust crate `ridl-rt` holds the same three values in `ridl_rt::contract`, and
 * a test of `ridl-transport-ws` keeps the two sides equal.
 */
export const SIGNAL_EVENT_CEILING = 3_840;
/**
 * The default ceiling of a `command` and a `query`, in bytes of encoded
 * payload: four 4,096-byte pages minus 256 bytes of header. See
 * {@link SIGNAL_EVENT_CEILING}.
 */
export const CALL_CEILING = 16_128;
/**
 * The default ceiling of a `fixed`, in bytes of encoded payload: sixteen
 * 4,096-byte pages minus 256 bytes of header. See
 * {@link SIGNAL_EVENT_CEILING}.
 */
export const FIXED_CEILING = 65_280;

/** The default ceiling of an interaction of `kind`, in bytes of encoded payload. */
export function defaultCeiling(kind: Kind): number {
  switch (kind) {
    case Kind.Signal:
    case Kind.Event:
      return SIGNAL_EVENT_CEILING;
    case Kind.Command:
    case Kind.Query:
      return CALL_CEILING;
    case Kind.Fixed:
      return FIXED_CEILING;
  }
}

/** The encodings an `attach` can ask for. */
export type EncodingName = "flatbuffers" | "proto3" | "podbuf";

/** The encoding tag of an encoding name: `flatbuffers` 1, `proto3` 2, `podbuf` 3. */
export function encodingTag(name: EncodingName): 1 | 2 | 3 {
  switch (name) {
    case "flatbuffers":
      return 1;
    case "proto3":
      return 2;
    case "podbuf":
      return 3;
  }
}

/** The interaction a frame is about. */
export interface Target {
  /** The interaction kind. */
  kind: Kind;
  /** The interface number, 1 or more. */
  iface: number;
  /** The member ordinal, 1 or more. */
  ord: number;
}

/** The sender's envelope: a signed 64-bit stamp and an unsigned 64-bit sequence. */
export interface Envelope {
  /** The timestamp, a signed 64-bit integer. */
  stamp: bigint;
  /** The sequence number, an unsigned 64-bit integer. */
  seq: bigint;
}

/** The rule a value broke, with the `detail` codes 1 to 6 in this order. */
export type Rule =
  | "range"
  | "step"
  | "length"
  | "pattern"
  | "variant"
  | "unique";

/** A contract error carried by an `answer` or a `response`. */
export type Contract =
  | { category: "invalid-value"; rule: Rule }
  | { category: "precondition-failed" }
  | { category: "contract-broken" }
  | { category: "unknown-interaction" };

/** Why a provider refused an `attach`. */
export type Reason =
  | "unknown-catalog"
  | "catalog-mismatch"
  | "encoding-unsupported"
  | "frame-version-unsupported"
  | "range";

/** The provenance of a `publish`, with the payload that only `live` carries. */
export type PublishState =
  | { provenance: "init" }
  | { provenance: "live"; payload: Uint8Array }
  | { provenance: "invalid" };

/** The outcome of a `response`, with the payload that only `reply` carries. */
export type ResponseOutcome =
  | { outcome: "accepted" }
  | { outcome: "reply"; payload: Uint8Array }
  | { outcome: "contract"; contract: Contract }
  | { outcome: "corrupt" }
  | { outcome: "busy" };

/** How a stream ended, as a `stream_end` carries it. */
export type StreamOutcome =
  | { kind: "end" }
  | { kind: "interrupted" }
  | { kind: "corrupt" }
  | { kind: "busy" }
  | { kind: "contract"; contract: Contract };

/** One frame. */
export type Frame =
  | {
    form: "attach";
    frameVersion: number;
    encoding: number;
    hash: Uint8Array;
    name: string;
  }
  | {
    form: "attached";
    outcome: { outcome: "accepted" } | { outcome: "refused"; reason: Reason };
  }
  | { form: "subscribe" | "unsubscribe" | "read"; target: Target }
  | {
    form: "answer";
    target: Target;
    outcome: { outcome: "accepted" } | {
      outcome: "contract";
      contract: Contract;
    };
  }
  | {
    form: "publish";
    target: Target;
    envelope: Envelope;
    state: PublishState;
  }
  | {
    form: "occurrence" | "request";
    target: Target;
    envelope: Envelope;
    payload: Uint8Array;
  }
  | {
    form: "response";
    target: Target;
    envelope: Envelope;
    correlation: bigint;
    outcome: ResponseOutcome;
  }
  | {
    form: "block";
    target: Target;
    envelope: Envelope;
    /** The length of the whole payload in bytes, 1 to 4294967295. */
    total: number;
    /** The position of the block in the payload, from 0. */
    index: number;
    /** The bytes of the block: a view of the message. */
    payload: Uint8Array;
  }
  | {
    form: "chunk";
    target: Target;
    envelope: Envelope;
    /** The `seq` of the request, the caller's name for the call. */
    correlation: bigint;
    /** The stream of the call, from 0. */
    stream: number;
    /** The bytes of the chunk, possibly empty: a view of the message. */
    payload: Uint8Array;
  }
  | {
    form: "stream_end";
    target: Target;
    envelope: Envelope;
    /** The `seq` of the request, the caller's name for the call. */
    correlation: bigint;
    /** The stream of the call, from 0. */
    stream: number;
    outcome: StreamOutcome;
  }
  | {
    form: "stream_credit";
    target: Target;
    /** The `seq` of the request, the caller's name for the call. */
    correlation: bigint;
    /** The stream of the call, from 0. */
    stream: number;
    /** The number of chunks granted, 1 to 4294967295. */
    credit: number;
  };

/**
 * A message the receiver cannot read. The frame specification has the receiver
 * record the frame and close the connection with code 1002; this package
 * closes with no code, because the platform cannot send 1002 (see the
 * session module).
 */
export type Violation =
  | "too-short"
  | "control-too-long"
  | "unknown-code"
  | "outcome-not-allowed"
  | "nonzero-detail"
  | "zero-number"
  | "bad-name-length"
  | "bad-name"
  | "bad-block"
  | "bad-stream";

/** The result of {@link decode}. */
export type Decoded =
  | { ok: true; frame: Frame }
  | { ok: false; class: "violation"; violation: Violation }
  | { ok: false; class: "discard"; discard: "unexpected-payload" };

const FORM_ATTACH = 1;
const FORM_ATTACHED = 2;
const FORM_SUBSCRIBE = 3;
const FORM_ANSWER = 4;
const FORM_UNSUBSCRIBE = 5;
const FORM_READ = 6;
const FORM_PUBLISH = 7;
const FORM_OCCURRENCE = 8;
const FORM_REQUEST = 9;
const FORM_RESPONSE = 10;
/** The form code of `chunk`, which frame versions 1 and 2 do not have. */
export const FORM_CHUNK = 12;
/** The form code of `stream_end`, which frame versions 1 and 2 do not have. */
export const FORM_STREAM_END = 13;
/** The form code of `stream_credit`, which frame versions 1 and 2 do not have. */
export const FORM_STREAM_CREDIT = 14;

const OUT_ACCEPTED = 1;
const OUT_REPLY = 2;
const OUT_INVALID_VALUE = 3;
const OUT_PRECONDITION_FAILED = 4;
const OUT_CONTRACT_BROKEN = 5;
const OUT_UNKNOWN_INTERACTION = 6;
const OUT_CORRUPT = 7;
const OUT_BUSY = 8;
const OUT_REFUSED = 9;
const OUT_END = 10;
const OUT_INTERRUPTED = 11;

const PROV_INIT = 1;
const PROV_LIVE = 2;
const PROV_INVALID = 3;

/** The common header fields: `form`, `kind`, `interface`, `ordinal`. */
const COMMON = 10;
const ATTACHED_LEN = 3;
const ANSWER_LEN = COMMON + 2;
const PUBLISH_LEN = COMMON + ENVELOPE_BYTES + 1;
const DATA_LEN = COMMON + ENVELOPE_BYTES;
const RESPONSE_LEN = COMMON + ENVELOPE_BYTES + 8 + 2;
/** The header of a `block`: the common fields, the envelope, `total` and `index`. */
const BLOCK_LEN = COMMON + ENVELOPE_BYTES + 4 + 4;
/** The header of a `chunk`: the common fields, the envelope, `correlation` and `stream`. */
const CHUNK_LEN = COMMON + ENVELOPE_BYTES + 8 + 1;
/** The header of a `stream_end`: a `chunk`'s, then `outcome` and `detail`. */
const STREAM_END_LEN = CHUNK_LEN + 2;
/** The header of a `stream_credit`: the common fields, `correlation`, `stream` and `credit`. */
const STREAM_CREDIT_LEN = COMMON + 8 + 1 + 4;
/** `form`, `frame_version`, `encoding`, `hash` and `name_len`. */
const ATTACH_FIXED = 1 + 4 + 1 + 32 + 1;

const RULES: readonly Rule[] = [
  "range",
  "step",
  "length",
  "pattern",
  "variant",
  "unique",
];
const REASONS: readonly Reason[] = [
  "unknown-catalog",
  "catalog-mismatch",
  "encoding-unsupported",
  "frame-version-unsupported",
  "range",
];

/** Thrown to carry a violation or a discard out of the decoder. */
class Failure {
  constructor(readonly result: Exclude<Decoded, { ok: true }>) {}
}

function fail(violation: Violation): never {
  throw new Failure({ ok: false, class: "violation", violation });
}

function need(msg: Uint8Array, len: number): void {
  if (msg.length < len) fail("too-short");
}

function control(msg: Uint8Array, len: number): void {
  need(msg, len);
  if (msg.length > len) fail("control-too-long");
}

function outcomeKnown(code: number): void {
  if (code < OUT_ACCEPTED || code > OUT_REFUSED) fail("unknown-code");
}

function contractFrom(outcome: number, detail: number): Contract {
  if (outcome === OUT_INVALID_VALUE) {
    const rule = RULES[detail - 1];
    if (rule === undefined) fail("unknown-code");
    return { category: "invalid-value", rule };
  }
  if (detail !== 0) fail("nonzero-detail");
  switch (outcome) {
    case OUT_PRECONDITION_FAILED:
      return { category: "precondition-failed" };
    case OUT_CONTRACT_BROKEN:
      return { category: "contract-broken" };
    default:
      return { category: "unknown-interaction" };
  }
}

function contractCodes(contract: Contract): [number, number] {
  switch (contract.category) {
    case "invalid-value":
      return [OUT_INVALID_VALUE, RULES.indexOf(contract.rule) + 1];
    case "precondition-failed":
      return [OUT_PRECONDITION_FAILED, 0];
    case "contract-broken":
      return [OUT_CONTRACT_BROKEN, 0];
    case "unknown-interaction":
      return [OUT_UNKNOWN_INTERACTION, 0];
  }
}

function streamCodes(outcome: StreamOutcome): [number, number] {
  switch (outcome.kind) {
    case "end":
      return [OUT_END, 0];
    case "interrupted":
      return [OUT_INTERRUPTED, 0];
    case "corrupt":
      return [OUT_CORRUPT, 0];
    case "busy":
      return [OUT_BUSY, 0];
    case "contract":
      return contractCodes(outcome.contract);
  }
}

/** A `stream_end` outcome from its `outcome` and `detail` codes. */
function streamFrom(code: number, detail: number): StreamOutcome {
  if (code >= OUT_INVALID_VALUE && code <= OUT_UNKNOWN_INTERACTION) {
    return { kind: "contract", contract: contractFrom(code, detail) };
  }
  if (code === OUT_ACCEPTED || code === OUT_REPLY || code === OUT_REFUSED) {
    return fail("outcome-not-allowed");
  }
  if (
    code !== OUT_CORRUPT && code !== OUT_BUSY && code !== OUT_END &&
    code !== OUT_INTERRUPTED
  ) {
    return fail("unknown-code");
  }
  if (detail !== 0) fail("nonzero-detail");
  switch (code) {
    case OUT_CORRUPT:
      return { kind: "corrupt" };
    case OUT_BUSY:
      return { kind: "busy" };
    case OUT_END:
      return { kind: "end" };
    default:
      return { kind: "interrupted" };
  }
}

/** The target of a stream frame, whose `kind` is `Command` or `Query`. */
function streamTargetAt(msg: Uint8Array, view: DataView): Target {
  const target = targetAt(msg, view);
  if (target.kind !== Kind.Command && target.kind !== Kind.Query) {
    fail("bad-stream");
  }
  return target;
}

function targetAt(msg: Uint8Array, view: DataView): Target {
  const kind = msg[1];
  if (kind < Kind.Signal || kind > Kind.Fixed) fail("unknown-code");
  const iface = view.getUint32(2, true);
  const ord = view.getUint32(6, true);
  if (iface === 0 || ord === 0) fail("zero-number");
  return { kind: kind as Kind, iface, ord };
}

function envelopeAt(view: DataView, at: number): Envelope {
  return {
    stamp: view.getBigInt64(at, true),
    seq: view.getBigUint64(at + 8, true),
  };
}

/**
 * Decodes one message. The function reads the message without session context:
 * the order of frames within a session is the session's to check. Payloads in
 * the result are views of `message`, not copies.
 */
export function decode(message: Uint8Array): Decoded {
  try {
    return { ok: true, frame: decodeFrame(message) };
  } catch (e) {
    if (e instanceof Failure) return e.result;
    throw e;
  }
}

function decodeFrame(msg: Uint8Array): Frame {
  if (msg.length === 0) fail("too-short");
  const view = new DataView(msg.buffer, msg.byteOffset, msg.byteLength);
  const form = msg[0];
  switch (form) {
    case FORM_ATTACH:
      return decodeAttach(msg, view);
    case FORM_ATTACHED: {
      control(msg, ATTACHED_LEN);
      outcomeKnown(msg[1]);
      if (msg[1] === OUT_ACCEPTED) {
        if (msg[2] !== 0) fail("nonzero-detail");
        return { form: "attached", outcome: { outcome: "accepted" } };
      }
      if (msg[1] === OUT_REFUSED) {
        const reason = REASONS[msg[2] - 1];
        if (reason === undefined) fail("unknown-code");
        return { form: "attached", outcome: { outcome: "refused", reason } };
      }
      return fail("outcome-not-allowed");
    }
    case FORM_SUBSCRIBE:
    case FORM_UNSUBSCRIBE:
    case FORM_READ: {
      control(msg, COMMON);
      const target = targetAt(msg, view);
      return {
        form: form === FORM_SUBSCRIBE
          ? "subscribe"
          : form === FORM_UNSUBSCRIBE
          ? "unsubscribe"
          : "read",
        target,
      };
    }
    case FORM_ANSWER: {
      control(msg, ANSWER_LEN);
      const target = targetAt(msg, view);
      const code = msg[10];
      const detail = msg[11];
      outcomeKnown(code);
      if (code === OUT_ACCEPTED) {
        if (detail !== 0) fail("nonzero-detail");
        return { form: "answer", target, outcome: { outcome: "accepted" } };
      }
      if (code >= OUT_INVALID_VALUE && code <= OUT_UNKNOWN_INTERACTION) {
        return {
          form: "answer",
          target,
          outcome: {
            outcome: "contract",
            contract: contractFrom(code, detail),
          },
        };
      }
      return fail("outcome-not-allowed");
    }
    case FORM_PUBLISH: {
      need(msg, PUBLISH_LEN);
      const target = targetAt(msg, view);
      const envelope = envelopeAt(view, COMMON);
      const rest = msg.subarray(PUBLISH_LEN);
      let state: PublishState;
      switch (msg[PUBLISH_LEN - 1]) {
        case PROV_INIT:
          state = { provenance: "init" };
          break;
        case PROV_LIVE:
          return {
            form: "publish",
            target,
            envelope,
            state: { provenance: "live", payload: rest },
          };
        case PROV_INVALID:
          state = { provenance: "invalid" };
          break;
        default:
          return fail("unknown-code");
      }
      if (rest.length > 0) discard();
      return { form: "publish", target, envelope, state };
    }
    case FORM_OCCURRENCE:
    case FORM_REQUEST: {
      need(msg, DATA_LEN);
      const target = targetAt(msg, view);
      const envelope = envelopeAt(view, COMMON);
      const payload = msg.subarray(DATA_LEN);
      return {
        form: form === FORM_OCCURRENCE ? "occurrence" : "request",
        target,
        envelope,
        payload,
      };
    }
    case FORM_RESPONSE: {
      need(msg, RESPONSE_LEN);
      const target = targetAt(msg, view);
      const envelope = envelopeAt(view, COMMON);
      const correlation = view.getBigUint64(COMMON + ENVELOPE_BYTES, true);
      const code = msg[RESPONSE_LEN - 2];
      const detail = msg[RESPONSE_LEN - 1];
      const rest = msg.subarray(RESPONSE_LEN);
      outcomeKnown(code);
      let outcome: ResponseOutcome;
      if (code === OUT_REFUSED) {
        return fail("outcome-not-allowed");
      } else if (code >= OUT_INVALID_VALUE && code <= OUT_UNKNOWN_INTERACTION) {
        outcome = { outcome: "contract", contract: contractFrom(code, detail) };
      } else if (detail !== 0) {
        return fail("nonzero-detail");
      } else if (code === OUT_ACCEPTED) {
        outcome = { outcome: "accepted" };
      } else if (code === OUT_REPLY) {
        return {
          form: "response",
          target,
          envelope,
          correlation,
          outcome: { outcome: "reply", payload: rest },
        };
      } else if (code === OUT_CORRUPT) {
        outcome = { outcome: "corrupt" };
      } else {
        outcome = { outcome: "busy" };
      }
      if (rest.length > 0) discard();
      return { form: "response", target, envelope, correlation, outcome };
    }
    case FORM_BLOCK:
      return decodeBlock(msg, view);
    case FORM_CHUNK: {
      need(msg, CHUNK_LEN);
      const target = streamTargetAt(msg, view);
      return {
        form: "chunk",
        target,
        envelope: envelopeAt(view, COMMON),
        correlation: view.getBigUint64(COMMON + ENVELOPE_BYTES, true),
        stream: msg[CHUNK_LEN - 1],
        payload: msg.subarray(CHUNK_LEN),
      };
    }
    case FORM_STREAM_END: {
      need(msg, STREAM_END_LEN);
      const target = streamTargetAt(msg, view);
      const outcome = streamFrom(
        msg[STREAM_END_LEN - 2],
        msg[STREAM_END_LEN - 1],
      );
      if (msg.length > STREAM_END_LEN) fail("bad-stream");
      return {
        form: "stream_end",
        target,
        envelope: envelopeAt(view, COMMON),
        correlation: view.getBigUint64(COMMON + ENVELOPE_BYTES, true),
        stream: msg[STREAM_END_LEN - 3],
        outcome,
      };
    }
    case FORM_STREAM_CREDIT: {
      need(msg, STREAM_CREDIT_LEN);
      const target = streamTargetAt(msg, view);
      const credit = view.getUint32(STREAM_CREDIT_LEN - 4, true);
      if (credit === 0) fail("zero-number");
      if (msg.length > STREAM_CREDIT_LEN) fail("bad-stream");
      return {
        form: "stream_credit",
        target,
        correlation: view.getBigUint64(COMMON, true),
        stream: msg[STREAM_CREDIT_LEN - 5],
        credit,
      };
    }
    default:
      return fail("unknown-code");
  }
}

/**
 * Reads a `block` (binding version 2). Every error of the block alone is a
 * violation, checked in this order after the header length, the kind range and
 * the zero numbers: the `kind` is `Fixed`, `total` is at least 1, `index` is
 * below the block count, and the bytes after the header are as many as the
 * position of the block gives (4,096 for every block but the last, the rest of
 * `total` for the last). The arithmetic stays in doubles: the largest value,
 * `BLOCK_BYTES * index`, is below 2^32.
 */
function decodeBlock(msg: Uint8Array, view: DataView): Frame {
  need(msg, BLOCK_LEN);
  const target = targetAt(msg, view);
  if (target.kind !== Kind.Fixed) fail("bad-block");
  const envelope = envelopeAt(view, COMMON);
  const total = view.getUint32(COMMON + ENVELOPE_BYTES, true);
  const index = view.getUint32(COMMON + ENVELOPE_BYTES + 4, true);
  if (total < 1) fail("bad-block");
  const count = blockCount(total);
  if (index >= count) fail("bad-block");
  const expected = index < count - 1
    ? BLOCK_BYTES
    : total - BLOCK_BYTES * index;
  if (msg.length - BLOCK_LEN !== expected) fail("bad-block");
  return {
    form: "block",
    target,
    envelope,
    total,
    index,
    payload: msg.subarray(BLOCK_LEN),
  };
}

function discard(): never {
  throw new Failure({
    ok: false,
    class: "discard",
    discard: "unexpected-payload",
  });
}

function decodeAttach(msg: Uint8Array, view: DataView): Frame {
  need(msg, ATTACH_FIXED);
  const nameLen = msg[ATTACH_FIXED - 1];
  if (nameLen === 0 || msg.length !== ATTACH_FIXED + nameLen) {
    fail("bad-name-length");
  }
  let name: string;
  try {
    // `ignoreBOM` keeps a leading U+FEFF in the name instead of dropping it.
    name = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(
      msg.subarray(ATTACH_FIXED),
    );
  } catch {
    return fail("bad-name");
  }
  if (msg[5] < 1 || msg[5] > 3) fail("unknown-code");
  return {
    form: "attach",
    frameVersion: view.getUint32(1, true),
    encoding: msg[5],
    hash: msg.subarray(6, 38),
    name,
  };
}

/** Why {@link attach} refused to build an `attach` frame. */
export class AttachError extends Error {
  /** `empty`: no name; `too-long`: over 255 UTF-8 bytes; `unknown-encoding`: tag outside 1 to 3. */
  readonly reason: "empty" | "too-long" | "unknown-encoding";

  constructor(reason: "empty" | "too-long" | "unknown-encoding") {
    super(`cannot build an attach frame: ${reason}`);
    this.name = "AttachError";
    this.reason = reason;
  }
}

const textEncoder = new TextEncoder();

/**
 * Builds an `attach` frame. The name must hold 1 to 255 bytes of UTF-8, the
 * encoding tag must be 1, 2 or 3, and the hash must be 32 bytes long.
 *
 * @throws {AttachError} for an empty or too long name or an unknown encoding.
 * @throws {RangeError} when the hash is not 32 bytes long.
 */
export function attach(
  frameVersion: number,
  encoding: number,
  hash: Uint8Array,
  name: string,
): Extract<Frame, { form: "attach" }> {
  if (encoding < 1 || encoding > 3 || !Number.isInteger(encoding)) {
    throw new AttachError("unknown-encoding");
  }
  if (hash.length !== 32) {
    throw new RangeError("the catalog hash must be 32 bytes long");
  }
  const length = textEncoder.encode(name).length;
  if (length === 0) throw new AttachError("empty");
  if (length > 255) throw new AttachError("too-long");
  return { form: "attach", frameVersion, encoding, hash, name };
}

function targetBytes(out: Uint8Array, view: DataView, form: number, t: Target) {
  out[0] = form;
  out[1] = t.kind;
  view.setUint32(2, t.iface, true);
  view.setUint32(6, t.ord, true);
}

function encodedLength(frame: Frame, nameBytes: Uint8Array): number {
  switch (frame.form) {
    case "attach":
      return ATTACH_FIXED + nameBytes.length;
    case "attached":
      return ATTACHED_LEN;
    case "subscribe":
    case "unsubscribe":
    case "read":
      return COMMON;
    case "answer":
      return ANSWER_LEN;
    case "publish":
      return PUBLISH_LEN +
        (frame.state.provenance === "live" ? frame.state.payload.length : 0);
    case "occurrence":
    case "request":
      return DATA_LEN + frame.payload.length;
    case "response":
      return RESPONSE_LEN +
        (frame.outcome.outcome === "reply" ? frame.outcome.payload.length : 0);
    case "block":
      return BLOCK_LEN + frame.payload.length;
    case "chunk":
      return CHUNK_LEN + frame.payload.length;
    case "stream_end":
      return STREAM_END_LEN;
    case "stream_credit":
      return STREAM_CREDIT_LEN;
  }
}

/**
 * Writes one frame as one message. The `detail` byte is the rule code under
 * `contract(invalid-value)` and 0 everywhere else.
 *
 * @throws {RangeError} for an `attach` frame whose name is empty or longer
 * than 255 UTF-8 bytes, or whose hash is not 32 bytes long.
 */
export function encode(frame: Frame): Uint8Array {
  const nameBytes = frame.form === "attach"
    ? textEncoder.encode(frame.name)
    : new Uint8Array(0);
  const out = new Uint8Array(encodedLength(frame, nameBytes));
  const view = new DataView(out.buffer);
  switch (frame.form) {
    case "attach":
      if (nameBytes.length === 0 || nameBytes.length > 255) {
        throw new RangeError("the unit name must hold 1 to 255 UTF-8 bytes");
      }
      if (frame.hash.length !== 32) {
        throw new RangeError("the catalog hash must be 32 bytes long");
      }
      out[0] = FORM_ATTACH;
      view.setUint32(1, frame.frameVersion, true);
      out[5] = frame.encoding;
      out.set(frame.hash, 6);
      out[38] = nameBytes.length;
      out.set(nameBytes, ATTACH_FIXED);
      break;
    case "attached":
      out[0] = FORM_ATTACHED;
      if (frame.outcome.outcome === "accepted") {
        out[1] = OUT_ACCEPTED;
      } else {
        out[1] = OUT_REFUSED;
        out[2] = REASONS.indexOf(frame.outcome.reason) + 1;
      }
      break;
    case "subscribe":
      targetBytes(out, view, FORM_SUBSCRIBE, frame.target);
      break;
    case "unsubscribe":
      targetBytes(out, view, FORM_UNSUBSCRIBE, frame.target);
      break;
    case "read":
      targetBytes(out, view, FORM_READ, frame.target);
      break;
    case "answer": {
      targetBytes(out, view, FORM_ANSWER, frame.target);
      const [code, detail] = frame.outcome.outcome === "accepted"
        ? [OUT_ACCEPTED, 0]
        : contractCodes(frame.outcome.contract);
      out[10] = code;
      out[11] = detail;
      break;
    }
    case "publish": {
      targetBytes(out, view, FORM_PUBLISH, frame.target);
      writeEnvelope(view, frame.envelope);
      const state = frame.state;
      if (state.provenance === "live") {
        out[PUBLISH_LEN - 1] = PROV_LIVE;
        out.set(state.payload, PUBLISH_LEN);
      } else {
        out[PUBLISH_LEN - 1] = state.provenance === "init"
          ? PROV_INIT
          : PROV_INVALID;
      }
      break;
    }
    case "occurrence":
    case "request":
      targetBytes(
        out,
        view,
        frame.form === "occurrence" ? FORM_OCCURRENCE : FORM_REQUEST,
        frame.target,
      );
      writeEnvelope(view, frame.envelope);
      out.set(frame.payload, DATA_LEN);
      break;
    case "response": {
      targetBytes(out, view, FORM_RESPONSE, frame.target);
      writeEnvelope(view, frame.envelope);
      view.setBigUint64(COMMON + ENVELOPE_BYTES, frame.correlation, true);
      const o = frame.outcome;
      let code: number;
      let detail = 0;
      switch (o.outcome) {
        case "accepted":
          code = OUT_ACCEPTED;
          break;
        case "reply":
          code = OUT_REPLY;
          out.set(o.payload, RESPONSE_LEN);
          break;
        case "contract":
          [code, detail] = contractCodes(o.contract);
          break;
        case "corrupt":
          code = OUT_CORRUPT;
          break;
        case "busy":
          code = OUT_BUSY;
          break;
      }
      out[RESPONSE_LEN - 2] = code;
      out[RESPONSE_LEN - 1] = detail;
      break;
    }
    case "block":
      targetBytes(out, view, FORM_BLOCK, frame.target);
      writeEnvelope(view, frame.envelope);
      view.setUint32(COMMON + ENVELOPE_BYTES, frame.total, true);
      view.setUint32(COMMON + ENVELOPE_BYTES + 4, frame.index, true);
      out.set(frame.payload, BLOCK_LEN);
      break;
    case "chunk":
      targetBytes(out, view, FORM_CHUNK, frame.target);
      writeEnvelope(view, frame.envelope);
      view.setBigUint64(COMMON + ENVELOPE_BYTES, frame.correlation, true);
      out[CHUNK_LEN - 1] = frame.stream;
      out.set(frame.payload, CHUNK_LEN);
      break;
    case "stream_end": {
      targetBytes(out, view, FORM_STREAM_END, frame.target);
      writeEnvelope(view, frame.envelope);
      view.setBigUint64(COMMON + ENVELOPE_BYTES, frame.correlation, true);
      out[STREAM_END_LEN - 3] = frame.stream;
      const [code, detail] = streamCodes(frame.outcome);
      out[STREAM_END_LEN - 2] = code;
      out[STREAM_END_LEN - 1] = detail;
      break;
    }
    case "stream_credit":
      targetBytes(out, view, FORM_STREAM_CREDIT, frame.target);
      view.setBigUint64(COMMON, frame.correlation, true);
      out[STREAM_CREDIT_LEN - 5] = frame.stream;
      view.setUint32(STREAM_CREDIT_LEN - 4, frame.credit, true);
      break;
  }
  return out;
}

function writeEnvelope(view: DataView, e: Envelope): void {
  view.setBigInt64(COMMON, e.stamp, true);
  view.setBigUint64(COMMON + 8, e.seq, true);
}
