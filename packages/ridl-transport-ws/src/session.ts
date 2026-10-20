/**
 * The consuming side of a session over the WebSocket binding (frame
 * specification §3, §5, §6, §7 and §11.1).
 *
 * {@link connect} opens a connection, sends the `attach` and resolves with a
 * {@link Session} once the provider answers `attached(accepted)`. The session
 * applies the consumer's rules that need no catalog and no payload decoding:
 * the held sample of each subscribed signal, the order of a signal's
 * publications, the sequence of an event's occurrences, and the end of the
 * session.
 *
 * @module
 */

import {
  attach,
  decode,
  encode,
  encodingTag,
  FRAME_VERSION,
  Kind,
  SUBPROTOCOL,
} from "./frame.ts";
import type { EncodingName, Envelope, Frame, Reason, Target } from "./frame.ts";
import {
  AttachRefused,
  ProtocolError,
  SessionError,
  type SessionViolation,
} from "./errors.ts";
import { Calls } from "./calls.ts";

/** The state of a signal or a `fixed`, as one `publish` carried it. */
export interface Sample {
  /** `init` before the first publication, `live` for a value, `invalid` for an invalidation. */
  provenance: "init" | "live" | "invalid";
  /** The envelope of the `publish`. */
  envelope: Envelope;
  /** The value under `live`, and `null` under `init` and `invalid`. */
  payload: Uint8Array | null;
}

/** One occurrence of a subscribed event. */
export interface Occurrence {
  /** The interface number. */
  iface: number;
  /** The member ordinal. */
  ord: number;
  /** The envelope of the `occurrence`. */
  envelope: Envelope;
  /** The value, as the provider encoded it. */
  payload: Uint8Array;
}

/**
 * What the consumer records about the session. A `violation` ends the
 * session; a `discarded` frame and a `loss` do not.
 */
export type Note =
  | {
    note: "violation";
    violation: SessionViolation;
  }
  | {
    note: "discarded";
    reason:
      | "unexpected-payload"
      | "stale"
      | "not-subscribed"
      | "unknown-correlation"
      | "unexpected-outcome"
      | "unexpected-answer";
    /** The frame, when the codec could read it. */
    frame?: Frame;
  }
  | {
    note: "loss";
    iface: number;
    ord: number;
    /** The `seq` that follows the last accepted occurrence. */
    expected: bigint;
    /** The `seq` that arrived. */
    received: bigint;
  };

/** The options of {@link connect}. */
export interface ConnectOptions {
  /** The catalog the session is bound to: the unit name and its 32-byte hash. */
  catalog: { name: string; hash: Uint8Array };
  /** The payload encoding the `attach` asks for. The default is `proto3`. */
  encoding?: EncodingName;
  /** Called for every accepted `publish` of a subscribed signal. */
  onPublish?: (target: Target, sample: Sample) => void;
  /** Called for every accepted occurrence, in arrival order. */
  onOccurrence?: (occurrence: Occurrence) => void;
  /** Called for every violation, discarded frame and loss. */
  onNote?: (note: Note) => void;
  /**
   * The caller's clock in microseconds, used as the `stamp` of a `request`.
   * The default is `Date.now()` times 1000: UTC, which differs from TAI by the
   * leap seconds.
   */
  clock?: () => bigint;
  /** Creates the socket. The default is `new WebSocket(url, protocols)`. */
  webSocket?: (url: string, protocols: string[]) => WebSocket;
}

/** How a session ended. */
export interface Ended {
  /** The close code the platform reported; 1006 when the connection failed. */
  code: number;
  /** The close reason the platform reported. */
  reason: string;
  /** Set when the provider refused the `attach`. */
  refused?: Reason;
  /** Set when the consumer ended the session for a violation. */
  violation?: string;
}

/**
 * Opens a session: validates the catalog name and hash, opens the connection
 * with the subprotocol `ridl-ws-v1`, sends the `attach` and resolves once the
 * provider answers `attached(accepted)`.
 *
 * The promise rejects with:
 * - an `AttachError` or a `RangeError` from `attach`, before any socket is
 *   created, for an empty name, a name longer than 255 UTF-8 bytes, an
 *   unknown encoding or a hash that is not 32 bytes long;
 * - a {@link ProtocolError} when the server broke the binding before
 *   `attached`. In Deno, a connection on which the server selected no
 *   subprotocol rejects with `ProtocolError('no-subprotocol')`; in a browser,
 *   the platform fails the handshake itself, so the promise rejects with a
 *   {@link SessionError} `down` (close 1006);
 * - an {@link AttachRefused} when the provider refused the `attach`. The
 *   client then closes the connection with code 1000, and
 *   {@link AttachRefused.closed} resolves on the close event;
 * - a {@link SessionError} with category `down` when the connection closed or
 *   failed first.
 */
export function connect(
  url: string,
  options: ConnectOptions,
): Promise<Session> {
  let first: Frame;
  try {
    first = attach(
      FRAME_VERSION,
      encodingTag(options.encoding ?? "proto3"),
      options.catalog.hash,
      options.catalog.name,
    );
  } catch (e) {
    return Promise.reject(e);
  }
  const create = options.webSocket ??
    ((u: string, p: string[]) => new WebSocket(u, p));
  let socket: WebSocket;
  try {
    socket = create(url, [SUBPROTOCOL]);
  } catch (e) {
    // For example a `SyntaxError` for an invalid URL.
    return Promise.reject(e);
  }
  return new Promise((resolve, reject) => {
    new Session(socket, first, options, { resolve, reject });
  });
}

type Waiting =
  | {
    op: "subscribe";
    /** Set when an `unsubscribe` was sent before this subscribe's answer. */
    dropped: boolean;
    resolve: () => void;
    reject: (e: SessionError) => void;
  }
  | {
    op: "read";
    resolve: (sample: Sample) => void;
    reject: (e: SessionError) => void;
  };

interface Opening {
  resolve: (session: Session) => void;
  reject: (e: unknown) => void;
}

const DOWN = { category: "down" } as const;

function channel(iface: number, ord: number): string {
  return `${iface}:${ord}`;
}

function identity(t: Target): string {
  return `${t.kind}:${t.iface}:${t.ord}`;
}

/** The largest interface number and member ordinal: 2^32 - 1. */
const MAX_NUMBER = 4_294_967_295;

/**
 * Throws a `RangeError` when `iface` or `ord` is not an integer from 1 to
 * 4294967295, the numbers a frame can carry (frame specification §4).
 */
function checkNumbers(iface: number, ord: number): void {
  checkNumber("interface number", iface);
  checkNumber("ordinal", ord);
}

function checkNumber(name: string, n: number): void {
  if (!Number.isInteger(n) || n < 1 || n > MAX_NUMBER) {
    throw new RangeError(
      `the ${name} must be an integer from 1 to ${MAX_NUMBER}, not ${n}`,
    );
  }
}

/** An open session. Created by {@link connect}. */
export class Session {
  /**
   * Resolves once, when the connection has closed or failed. After
   * {@link Session.close} it resolves when the closing handshake completes;
   * the waiting operations were already rejected with `down` by `close`.
   */
  readonly closed: Promise<Ended>;

  #socket: WebSocket;
  #options: ConnectOptions;
  #opening: Opening | null;
  #attachFrame: Frame;
  #attached = false;
  #ended = false;
  #closeSent = false;
  #refused: Reason | undefined;
  #violation: SessionViolation | undefined;
  #resolveClosed: ((ended: Ended) => void) | null = null;
  /** The `subscribe` and `read` operations waiting, per `(kind, iface, ord)`. */
  #waiting = new Map<string, Waiting[]>();
  /** The subscribed signals, keyed `iface:ord`. */
  #signals = new Set<string>();
  /** The held sample of each subscribed signal that has received one. */
  #held = new Map<string, Sample>();
  /** The subscribed events, keyed `iface:ord`. */
  #events = new Set<string>();
  /** The last accepted `seq` of each subscribed event. */
  #lastSeq = new Map<string, bigint>();
  /** The commands and queries in flight. */
  #calls = new Calls();

  /** @internal Use {@link connect}. */
  constructor(
    socket: WebSocket,
    attachFrame: Frame,
    options: ConnectOptions,
    opening: Opening,
  ) {
    this.#socket = socket;
    this.#attachFrame = attachFrame;
    this.#options = options;
    this.#opening = opening;
    this.closed = new Promise((resolve) => {
      this.#resolveClosed = resolve;
    });
    socket.binaryType = "arraybuffer";
    socket.onopen = () => this.#onOpen();
    socket.onmessage = (event) => this.#onMessage(event);
    socket.onclose = (event) => this.#onEnd(event.code, event.reason);
    socket.onerror = () => this.#onEnd(1006, "");
  }

  /**
   * Subscribes a signal or an event. Resolves on `answer(accepted)`; rejects
   * with a `contract` {@link SessionError} on a refusing `answer`. A target of
   * another kind is sent as given, and the provider refuses it. Rejects with a
   * `RangeError`, and sends nothing, when the interface number or the ordinal
   * is not an integer from 1 to 4294967295.
   */
  subscribe(target: Target): Promise<void> {
    return new Promise((resolve, reject) => {
      checkNumbers(target.iface, target.ord);
      if (this.#ended) return reject(new SessionError(DOWN));
      this.#enqueue(target, {
        op: "subscribe",
        dropped: false,
        resolve,
        reject,
      });
      this.#send({ form: "subscribe", target });
    });
  }

  /**
   * Ends a subscription and drops what the session held for it: the held
   * sample of a signal, or the last `seq` of an event. Frames of the
   * interaction that arrive later are discarded. A `subscribe` of the same
   * interaction still waiting for its answer resolves on that answer, but does
   * not make the interaction subscribed. Does nothing once the session has
   * ended.
   *
   * @throws {RangeError} when the interface number or the ordinal is not an
   * integer from 1 to 4294967295; nothing is sent.
   */
  unsubscribe(target: Target): void {
    checkNumbers(target.iface, target.ord);
    if (this.#ended) return;
    this.#send({ form: "unsubscribe", target });
    for (const w of this.#waiting.get(identity(target)) ?? []) {
      if (w.op === "subscribe") w.dropped = true;
    }
    const key = channel(target.iface, target.ord);
    if (target.kind === Kind.Signal) {
      this.#signals.delete(key);
      this.#held.delete(key);
    } else if (target.kind === Kind.Event) {
      this.#events.delete(key);
      this.#lastSeq.delete(key);
    }
  }

  /**
   * Reads a signal or a `fixed` once. Resolves with the sample of the next
   * `publish` of that interaction; rejects with a `contract`
   * {@link SessionError} on a refusing `answer`. A read does not subscribe:
   * the sample is held only when the signal is already subscribed. Rejects
   * with a `RangeError`, and sends nothing, when the interface number or the
   * ordinal is not an integer from 1 to 4294967295.
   */
  read(target: Target): Promise<Sample> {
    return new Promise((resolve, reject) => {
      checkNumbers(target.iface, target.ord);
      if (this.#ended) return reject(new SessionError(DOWN));
      this.#enqueue(target, { op: "read", resolve, reject });
      this.#send({ form: "read", target });
    });
  }

  /** The held sample of a subscribed signal, or `undefined` when there is none. */
  sample(iface: number, ord: number): Sample | undefined {
    return this.#held.get(channel(iface, ord));
  }

  /**
   * Sends a command: one `request` whose `seq` is the session's next caller
   * number, whose `stamp` is the `clock` option's value, and whose payload is
   * a copy of `args`. Resolves on `accepted`. Rejects with a
   * {@link SessionError}: `contract`, `corrupt` or `busy` as the provider
   * answered, `undelivered` when `opts.bound` milliseconds pass with no
   * `response`, `down` when the session has ended or ends first. Rejects with
   * a `RangeError`, and sends nothing, when `opts.bound` is not a finite number
   * of milliseconds greater than 0 and at most 2147483647, or when `iface` or
   * `ord` is not an integer from 1 to 4294967295.
   */
  command(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: { bound: number },
  ): Promise<void> {
    try {
      checkNumbers(iface, ord);
    } catch (e) {
      return Promise.reject(e);
    }
    return this.#calls.command(
      opts.bound,
      (seq) => this.#request(Kind.Command, iface, ord, seq, args),
    );
  }

  /**
   * Sends a query, as {@link Session.command} sends a command. Resolves with
   * the payload of the `reply`, an empty `Uint8Array` when the reply has no
   * bytes. Rejects as a command does, including the `RangeError` for a bound
   * that is not greater than 0 and at most 2147483647 milliseconds, except
   * that a passed bound rejects with `timeout`.
   */
  query(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: { bound: number },
  ): Promise<Uint8Array> {
    try {
      checkNumbers(iface, ord);
    } catch (e) {
      return Promise.reject(e);
    }
    return this.#calls.query(
      opts.bound,
      (seq) => this.#request(Kind.Query, iface, ord, seq, args),
    );
  }

  /**
   * Ends the session: sends a close with code 1000 and at once rejects every
   * waiting operation with `down`, as every later operation is. {@link
   * Session.closed} resolves later, when the closing handshake completes.
   */
  close(): void {
    if (!this.#closeSent) {
      this.#closeSent = true;
      this.#socket.close(1000);
    }
    this.#fail();
  }

  #send(frame: Frame): void {
    // `encode` allocates its own `ArrayBuffer`, which the DOM typing of
    // `send` requires.
    this.#socket.send(encode(frame) as Uint8Array<ArrayBuffer>);
  }

  #request(
    kind: typeof Kind.Command | typeof Kind.Query,
    iface: number,
    ord: number,
    seq: bigint,
    args: Uint8Array,
  ): void {
    const clock = this.#options.clock ?? (() => BigInt(Date.now()) * 1000n);
    // `encode` copies `args` into the message it allocates; the session keeps
    // no reference to `args`.
    this.#send({
      form: "request",
      target: { kind, iface, ord },
      envelope: { stamp: clock(), seq },
      payload: args,
    });
  }

  #note(note: Note): void {
    const onNote = this.#options.onNote;
    if (onNote) this.#listen(() => onNote(note));
  }

  /**
   * Calls a listener. An exception it throws is reported to the platform and
   * does not stop the handling of the frame. Callers update the session's
   * state before calling a listener.
   */
  #listen(call: () => void): void {
    try {
      call();
    } catch (e) {
      globalThis.reportError(e);
    }
  }

  #enqueue(target: Target, waiting: Waiting): void {
    const key = identity(target);
    const queue = this.#waiting.get(key);
    if (queue) queue.push(waiting);
    else this.#waiting.set(key, [waiting]);
  }

  #onOpen(): void {
    if (this.#ended) return;
    if (this.#socket.protocol !== SUBPROTOCOL) {
      this.#violate("no-subprotocol");
      return;
    }
    this.#send(this.#attachFrame);
  }

  #onMessage(event: MessageEvent): void {
    if (this.#ended) return;
    if (typeof event.data === "string") {
      this.#violate("text-message");
      return;
    }
    const decoded = decode(new Uint8Array(event.data as ArrayBuffer));
    if (!decoded.ok) {
      if (decoded.class === "violation") {
        this.#violate(decoded.violation);
      } else if (!this.#attached) {
        this.#violate("unexpected-form");
      } else {
        this.#note({ note: "discarded", reason: decoded.discard });
      }
      return;
    }
    const frame = decoded.frame;
    if (!this.#attached) {
      this.#onFirst(frame);
      return;
    }
    switch (frame.form) {
      case "answer":
        this.#onAnswer(frame);
        return;
      case "publish":
        this.#onPublish(frame);
        return;
      case "occurrence":
        this.#onOccurrence(frame);
        return;
      case "response": {
        const reason = this.#calls.settle(frame);
        if (reason) this.#note({ note: "discarded", reason, frame });
        return;
      }
      default:
        // A second `attached`, or a form only a consumer sends.
        this.#violate("unexpected-form");
    }
  }

  #onFirst(frame: Frame): void {
    if (frame.form !== "attached") {
      this.#violate("unexpected-form");
      return;
    }
    const opening = this.#opening;
    this.#opening = null;
    if (frame.outcome.outcome === "accepted") {
      this.#attached = true;
      opening?.resolve(this);
      return;
    }
    this.#refused = frame.outcome.reason;
    this.#ended = true;
    // The client closes the connection itself, so that a provider that does
    // not close leaves no connection open.
    if (!this.#closeSent) {
      this.#closeSent = true;
      this.#socket.close(1000);
    }
    opening?.reject(new AttachRefused(frame.outcome.reason, this.closed));
  }

  #onAnswer(frame: Extract<Frame, { form: "answer" }>): void {
    const key = identity(frame.target);
    const queue = this.#waiting.get(key);
    const head = queue?.[0];
    if (
      !queue || !head ||
      (frame.outcome.outcome === "accepted" && head.op !== "subscribe")
    ) {
      this.#note({ note: "discarded", reason: "unexpected-answer", frame });
      return;
    }
    queue.shift();
    if (queue.length === 0) this.#waiting.delete(key);
    if (frame.outcome.outcome === "contract") {
      head.reject(
        new SessionError({
          category: "contract",
          contract: frame.outcome.contract,
        }),
      );
      return;
    }
    if (head.op !== "subscribe") return; // ruled out above
    const t = frame.target;
    if (!head.dropped) {
      if (t.kind === Kind.Signal) this.#signals.add(channel(t.iface, t.ord));
      if (t.kind === Kind.Event) this.#events.add(channel(t.iface, t.ord));
    }
    head.resolve();
  }

  #onPublish(frame: Extract<Frame, { form: "publish" }>): void {
    const t = frame.target;
    const sample: Sample = {
      provenance: frame.state.provenance,
      envelope: frame.envelope,
      payload: frame.state.provenance === "live" ? frame.state.payload : null,
    };
    // Every `read` at the head of the identity's queue settles with this frame.
    const reads: Extract<Waiting, { op: "read" }>[] = [];
    const key = identity(t);
    const queue = this.#waiting.get(key);
    while (queue && queue[0]?.op === "read") {
      const read = queue.shift();
      if (read?.op === "read") reads.push(read);
    }
    if (queue && queue.length === 0) this.#waiting.delete(key);
    for (const r of reads) r.resolve(sample);

    const ch = channel(t.iface, t.ord);
    if (t.kind !== Kind.Signal || !this.#signals.has(ch)) {
      if (reads.length === 0) {
        this.#note({ note: "discarded", reason: "not-subscribed", frame });
      }
      return;
    }
    const held = this.#held.get(ch);
    // The server reuses a session `seq` only for an identical sample (the
    // crate's D-24), so a `seq` equal to the held one is the reply to a read
    // that a live publish already settled, or a duplicate: it is dropped and
    // not recorded.
    if (held && frame.envelope.seq === held.envelope.seq) return;
    if (held && frame.envelope.seq < held.envelope.seq) {
      // A frame that answered a read is not a discard, though it changes nothing.
      if (reads.length === 0) {
        this.#note({ note: "discarded", reason: "stale", frame });
      }
      return;
    }
    this.#held.set(ch, sample);
    const onPublish = this.#options.onPublish;
    if (onPublish) this.#listen(() => onPublish(t, sample));
  }

  #onOccurrence(
    frame: Extract<Frame, { form: "occurrence" | "request" }>,
  ): void {
    const t = frame.target;
    const ch = channel(t.iface, t.ord);
    if (t.kind !== Kind.Event || !this.#events.has(ch)) {
      this.#note({ note: "discarded", reason: "not-subscribed", frame });
      return;
    }
    const seq = frame.envelope.seq;
    const last = this.#lastSeq.get(ch);
    if (last !== undefined && seq <= last) {
      this.#note({ note: "discarded", reason: "stale", frame });
      return;
    }
    this.#lastSeq.set(ch, seq);
    if (last !== undefined && seq > last + 1n) {
      this.#note({
        note: "loss",
        iface: t.iface,
        ord: t.ord,
        expected: last + 1n,
        received: seq,
      });
    }
    const onOccurrence = this.#options.onOccurrence;
    if (onOccurrence) {
      const occurrence: Occurrence = {
        iface: t.iface,
        ord: t.ord,
        envelope: frame.envelope,
        payload: frame.payload,
      };
      this.#listen(() => onOccurrence(occurrence));
    }
  }

  /**
   * Ends the session for a violation: records it, closes without a code (the
   * platform cannot send 1002) and rejects everything waiting.
   */
  #violate(violation: SessionViolation): void {
    this.#violation = violation;
    if (!this.#closeSent) {
      this.#closeSent = true;
      this.#socket.close();
    }
    const opening = this.#opening;
    this.#opening = null;
    opening?.reject(new ProtocolError(violation));
    this.#fail();
    this.#note({ note: "violation", violation });
  }

  /**
   * Marks the session ended and rejects every waiting operation and every call
   * in flight with `down`.
   */
  #fail(): void {
    this.#ended = true;
    this.#calls.fail();
    const waiting = [...this.#waiting.values()].flat();
    this.#waiting.clear();
    this.#signals.clear();
    this.#held.clear();
    this.#events.clear();
    this.#lastSeq.clear();
    for (const w of waiting) w.reject(new SessionError(DOWN));
  }

  #onEnd(code: number, reason: string): void {
    const opening = this.#opening;
    this.#opening = null;
    opening?.reject(new SessionError(DOWN));
    this.#fail();
    const resolve = this.#resolveClosed;
    if (!resolve) return;
    this.#resolveClosed = null;
    const ended: Ended = { code, reason };
    if (this.#refused !== undefined) ended.refused = this.#refused;
    if (this.#violation !== undefined) ended.violation = this.#violation;
    resolve(ended);
  }
}
