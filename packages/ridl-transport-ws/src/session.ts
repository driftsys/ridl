/**
 * The consuming side of a session over the WebSocket binding (frame
 * specification §3, §5, §6, §7 and §11.1).
 *
 * {@link connect} opens a connection, sends the `attach` and resolves with a
 * {@link Session} once the provider answers `attached(accepted)`. The session
 * applies the consumer's rules that need no payload decoding: the held sample
 * of each subscribed signal, the order of a signal's publications, the
 * sequence of an event's occurrences, the reassembly of the `block` frames of
 * a `fixed`, the size of each received payload against its member's limit,
 * and the end of the session.
 *
 * @module
 */

import {
  attach,
  decode,
  defaultCeiling,
  encode,
  encodingTag,
  FRAME_VERSION,
  Kind,
  SUBPROTOCOL,
  SUBPROTOCOL_V1,
} from "./frame.ts";
import type { EncodingName, Envelope, Frame, Reason, Target } from "./frame.ts";
import {
  AttachRefused,
  ProtocolError,
  SessionError,
  type SessionViolation,
} from "./errors.ts";
import { Calls } from "./calls.ts";
import { type BlockFrame, fixedLimit, Reassembly } from "./reassembly.ts";

/** The state of a signal or a `fixed`, as one `publish` carried it. */
export interface Sample {
  /** `init` before the first publication, `live` for a value, `invalid` for an invalidation. */
  provenance: "init" | "live" | "invalid";
  /** The envelope of the `publish`. */
  envelope: Envelope;
  /** The value under `live`, and `null` under `init` and `invalid`. */
  payload: Uint8Array | null;
  /**
   * Set when the session itself invalidated the sample: the `publish` carried
   * a `live` payload longer than the member's limit. The provenance is then
   * `invalid` and `payload` is `null`; the envelope is the frame's.
   */
  detected?: "corrupt";
}

/** One occurrence of a subscribed event. */
export interface Occurrence {
  /** The interface number. */
  iface: number;
  /** The member ordinal. */
  ord: number;
  /** The envelope of the `occurrence`. */
  envelope: Envelope;
  /**
   * The value, as the provider encoded it; empty when the session refused it
   * (`detected` is set).
   */
  payload: Uint8Array;
  /**
   * Set when the session refused the value: the `occurrence` carried a
   * payload longer than the member's limit. The occurrence is delivered with
   * an empty payload, so that nothing fails silently.
   */
  detected?: "corrupt";
}

/**
 * One member of a generated interface table: the entry
 * `<Interface>.<member>` of a generated `<base>.codec.ts` module. `limit` is
 * the member's payload limit in bytes when the catalog records one; absent,
 * the member has the default ceiling of its kind.
 */
export interface MemberEntry {
  /** The interface number. */
  readonly iface: number;
  /** The member ordinal. */
  readonly ord: number;
  /** The interaction kind, as the generated module spells it. */
  readonly kind: "signal" | "event" | "command" | "query" | "fixed";
  /**
   * The payload limit a `[limits]` entry of the manifest gives the member:
   * its `[limits.interaction]` entry, else the entry of its kind in
   * `[limits.kind]`, else `default` clamped to the default ceiling of
   * its kind.
   */
  readonly limit?: number;
}

/**
 * A generated interface table: the object `export const <Interface>` of a
 * generated `<base>.codec.ts` module, whose `iface` is the interface number
 * and whose other properties are its members.
 */
export type InterfaceTable = {
  readonly [member: string]: MemberEntry | number;
};

/**
 * What the consumer records about the session. A `violation` ends the
 * session; a `discarded` frame and a `loss` do not.
 *
 * A `discarded` note with reason `over-limit` records a `live` payload of a
 * `fixed` longer than the member's limit, whether it arrived in one `publish`
 * or in `block` frames: `frame` is the `publish`, or the first block. The
 * reads it answers reject with `corrupt`, and the note is recorded whether or
 * not a read waited.
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
      | "unexpected-answer"
      | "over-limit";
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
  /**
   * The generated interface tables of the catalog, from which the session
   * reads each member's payload limit. A received payload longer than its
   * member's limit is refused before it reaches the application. A member no
   * table names, and every member when no table is given, has the default
   * ceiling of its kind.
   */
  interfaces?: readonly InterfaceTable[];
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
 * offering the subprotocols `ridl-ws-v2` and `ridl-ws-v1`, in that order,
 * sends the `attach` and resolves once the provider answers
 * `attached(accepted)`. The `attach` carries the frame version of the
 * subprotocol the server selected: 2 on `ridl-ws-v2`, 1 on `ridl-ws-v1`. On a
 * session of frame version 1, a message of the `block` form is a violation
 * `unknown-code`, as an unknown form.
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
  let first: AttachFrame;
  try {
    // `attach` builds an `attach` frame; its frame version is replaced by the
    // one of the selected subprotocol when the connection opens.
    first = attach(
      FRAME_VERSION,
      encodingTag(options.encoding ?? "proto3"),
      options.catalog.hash,
      options.catalog.name,
    ) as AttachFrame;
  } catch (e) {
    return Promise.reject(e);
  }
  const create = options.webSocket ??
    ((u: string, p: string[]) => new WebSocket(u, p));
  let socket: WebSocket;
  try {
    socket = create(url, [SUBPROTOCOL, SUBPROTOCOL_V1]);
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

type AttachFrame = Extract<Frame, { form: "attach" }>;

/** The form code of `block`, which frame version 1 does not have. */
const FORM_BLOCK = 11;

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

const KIND_CODES: Record<MemberEntry["kind"], Kind> = {
  signal: Kind.Signal,
  event: Kind.Event,
  command: Kind.Command,
  query: Kind.Query,
  fixed: Kind.Fixed,
};

/**
 * The limits the generated tables record, keyed by the identity of the
 * member. A member with no `limit` has no entry.
 */
function recordedLimits(
  tables: readonly InterfaceTable[] | undefined,
): Map<string, number> {
  const limits = new Map<string, number>();
  for (const table of tables ?? []) {
    for (const entry of Object.values(table)) {
      if (typeof entry !== "object" || entry.limit === undefined) continue;
      const kind = KIND_CODES[entry.kind];
      limits.set(
        identity({ kind, iface: entry.iface, ord: entry.ord }),
        entry.limit,
      );
    }
  }
  return limits;
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
  #attachFrame: AttachFrame;
  /**
   * The frame version of the session: 2 on `ridl-ws-v2`, 1 on `ridl-ws-v1`.
   * Set when the connection opens.
   */
  #frameVersion = FRAME_VERSION;
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
  /** The limits the generated tables record, keyed by member identity. */
  #limits: Map<string, number>;
  /** The `block` payload being collected or skipped. */
  #reassembly = new Reassembly();
  /**
   * The reads that the payload being collected answers: the reads at the
   * head of the identity's queue when its first block arrived.
   */
  #collected: Extract<Waiting, { op: "read" }>[] = [];

  /** @internal Use {@link connect}. */
  constructor(
    socket: WebSocket,
    attachFrame: AttachFrame,
    options: ConnectOptions,
    opening: Opening,
  ) {
    this.#socket = socket;
    this.#attachFrame = attachFrame;
    this.#options = options;
    this.#opening = opening;
    this.#limits = recordedLimits(options.interfaces);
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
   * ended, except that an interface number or an ordinal that is not an
   * integer from 1 to 4294967295 still throws.
   *
   * @throws {RangeError} when the interface number or the ordinal is not an
   * integer from 1 to 4294967295, also after the session has ended; nothing is
   * sent.
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
   * {@link SessionError} on a refusing `answer`. On a session of frame
   * version 2, the value of a `fixed` may arrive in `block` frames, which the
   * session reassembles into one sample; a `fixed` longer than its limit, in
   * one `publish` or in blocks, rejects with `corrupt`. A read does not
   * subscribe: the sample is held only when the signal is already subscribed.
   * Rejects with a `RangeError`, and sends nothing, when the interface number
   * or the ordinal is not an integer from 1 to 4294967295.
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
   * `response`, `down` when the session has ended or ends first; `corrupt`
   * also, before anything is sent, when `args` is longer than the member's
   * limit, the refusal the provider would answer. Rejects with a
   * `RangeError`, and sends nothing, when `opts.bound` is not a finite number
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
      this.#checkArgs({ kind: Kind.Command, iface, ord }, args);
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
    const target: Target = { kind: Kind.Query, iface, ord };
    try {
      checkNumbers(iface, ord);
      this.#checkArgs(target, args);
    } catch (e) {
      return Promise.reject(e);
    }
    return this.#calls.query(
      opts.bound,
      this.#limitOf(target),
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
   * The payload limit of a member, in bytes: the one the generated tables
   * record for it, else the default ceiling of its kind. The limit of a
   * `fixed` is held by `fixedLimit`, so that a whole `publish` and the blocks
   * of a payload are compared with the same value.
   */
  #limitOf(target: Target): number {
    const limit = this.#limits.get(identity(target)) ??
      defaultCeiling(target.kind);
    return target.kind === Kind.Fixed ? fixedLimit(limit) : limit;
  }

  /**
   * Throws the `corrupt` {@link SessionError} when the arguments of a call
   * are longer than the member's limit: the provider would answer the
   * `request` so, and a larger message could end the session instead, so the
   * call is refused before it is sent.
   */
  #checkArgs(target: Target, args: Uint8Array): void {
    if (args.length > this.#limitOf(target)) {
      throw new SessionError({ category: "corrupt" });
    }
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
    switch (this.#socket.protocol) {
      case SUBPROTOCOL:
        this.#frameVersion = FRAME_VERSION;
        break;
      case SUBPROTOCOL_V1:
        this.#frameVersion = 1;
        break;
      default:
        this.#violate("no-subprotocol");
        return;
    }
    this.#send({ ...this.#attachFrame, frameVersion: this.#frameVersion });
  }

  #onMessage(event: MessageEvent): void {
    if (this.#ended) return;
    if (typeof event.data === "string") {
      this.#violate("text-message");
      return;
    }
    const message = new Uint8Array(event.data as ArrayBuffer);
    // Frame version 1 has no `block` form: the form code is unknown there,
    // whatever follows it and whether or not the session is attached yet.
    if (this.#frameVersion === 1 && message[0] === FORM_BLOCK) {
      this.#violate("unknown-code");
      return;
    }
    const decoded = decode(message);
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
      case "publish":
        // The provider answers the reads of one identity in order, and
        // answers one read with one `publish` or with its blocks: a frame of
        // the `fixed` being reassembled is a defect of the sender.
        if (this.#reassembly.holds(frame.target)) {
          this.#violate("block-sequence");
        } else if (frame.form === "answer") {
          this.#onAnswer(frame);
        } else {
          this.#onPublish(frame);
        }
        return;
      case "block":
        this.#onBlock(frame);
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

  /**
   * Applies one `publish`. `reads` are the reads it answers; by default,
   * every `read` at the head of the identity's queue.
   */
  #onPublish(
    frame: Extract<Frame, { form: "publish" }>,
    reads: Extract<Waiting, { op: "read" }>[] = this.#takeReads(frame.target),
  ): void {
    const t = frame.target;
    // A `live` payload longer than the member's limit never reaches the
    // application: the sample is invalid with `detected`, and a `read` it
    // answers rejects `corrupt`.
    const oversized = frame.state.provenance === "live" &&
      frame.state.payload.length > this.#limitOf(t);
    const sample: Sample = oversized
      ? {
        provenance: "invalid",
        envelope: frame.envelope,
        payload: null,
        detected: "corrupt",
      }
      : {
        provenance: frame.state.provenance,
        envelope: frame.envelope,
        payload: frame.state.provenance === "live" ? frame.state.payload : null,
      };
    for (const r of reads) {
      if (oversized) r.reject(new SessionError({ category: "corrupt" }));
      else r.resolve(sample);
    }

    const ch = channel(t.iface, t.ord);
    if (t.kind !== Kind.Signal || !this.#signals.has(ch)) {
      if (oversized && t.kind === Kind.Fixed) {
        this.#note({ note: "discarded", reason: "over-limit", frame });
      } else if (reads.length === 0) {
        this.#note({ note: "discarded", reason: "not-subscribed", frame });
      }
      return;
    }
    const held = this.#held.get(ch);
    // The server reuses a session `seq` only for an identical sample (the
    // crate's D-24), except `Init`, which always carries `seq` 0. A `seq` equal
    // to the held one is the reply to a read that a live publish already
    // settled, or a duplicate: it is dropped and not recorded. This includes an
    // `Init` publish while an `Init` with `seq` 0 is held.
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

  /** Removes and returns every `read` at the head of the identity's queue. */
  #takeReads(t: Target): Extract<Waiting, { op: "read" }>[] {
    const reads: Extract<Waiting, { op: "read" }>[] = [];
    const key = identity(t);
    const queue = this.#waiting.get(key);
    while (queue && queue[0]?.op === "read") {
      const read = queue.shift();
      if (read?.op === "read") reads.push(read);
    }
    if (queue && queue.length === 0) this.#waiting.delete(key);
    return reads;
  }

  /**
   * Applies one `block`. A completed payload is handled as a `live`
   * `publish` that arrived whole, and settles the reads that were at the
   * head of the identity's queue when its first block arrived: a read sent
   * later waits for the next payload. A payload whose `total` is above the
   * member's limit rejects the reads at the head of the identity's queue with
   * `corrupt` at its first block, is recorded `over-limit`, and its other
   * blocks change nothing. A sequence error ends the session.
   */
  #onBlock(frame: BlockFrame): void {
    const t = frame.target;
    const step = this.#reassembly.accept(frame, this.#limitOf(t));
    switch (step.step) {
      case "complete": {
        const reads = frame.index === 0 ? this.#takeReads(t) : this.#collected;
        this.#collected = [];
        this.#onPublish({
          form: "publish",
          target: t,
          envelope: frame.envelope,
          state: { provenance: "live", payload: step.payload },
        }, reads);
        return;
      }
      case "over-limit":
        for (const r of this.#takeReads(t)) {
          r.reject(new SessionError({ category: "corrupt" }));
        }
        this.#note({ note: "discarded", reason: "over-limit", frame });
        return;
      case "sequence":
        this.#violate("block-sequence");
        return;
      case "collecting":
        if (frame.index === 0) this.#collected = this.#takeReads(t);
        return;
      case "skipping":
        return;
    }
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
      // A payload longer than the member's limit is not handed out: the
      // occurrence is delivered empty, with `detected` set.
      const occurrence: Occurrence = frame.payload.length > this.#limitOf(t)
        ? {
          iface: t.iface,
          ord: t.ord,
          envelope: frame.envelope,
          payload: new Uint8Array(),
          detected: "corrupt",
        }
        : {
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
    this.#reassembly.clear();
    this.#calls.fail();
    const waiting = [...this.#collected, ...this.#waiting.values()].flat();
    this.#collected = [];
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
