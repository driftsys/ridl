/**
 * The consuming side of a session over the WebSocket binding (frame
 * specification §3, §5, §6, §7 and §11.1).
 *
 * {@link connect} opens a connection, sends the `attach` and resolves with a
 * {@link Session} once the provider answers `attached(accepted)`. The session
 * applies the consumer's rules that need no payload decoding: the held sample
 * of each subscribed signal, the order of a signal's publications, the
 * sequence of an event's occurrences, the reassembly of the `block` frames of
 * a `fixed`, the order and the credit of the chunks of a streamed reply, the
 * size of each received payload against its member's limit, a caller's
 * per-signal payload check, and the end of the session.
 *
 * @module
 */

import {
  attach,
  decode,
  defaultCeiling,
  encode,
  encodingTag,
  FORM_CHUNK,
  FORM_STREAM_CREDIT,
  FORM_STREAM_END,
  FRAME_VERSION,
  FRAME_VERSION_3,
  Kind,
  SUBPROTOCOL,
  SUBPROTOCOL_V1,
  SUBPROTOCOL_V3,
} from "./frame.ts";
import type {
  EncodingName,
  Frame,
  Reason,
  StreamOutcome,
  Target,
} from "./frame.ts";
import type {
  CallOptions,
  Detection,
  RawOccurrence,
  RawSample,
  SignalCheck,
  SignalCheckEntry,
  WaitOptions,
} from "@driftsys/ridl-runtime";
import {
  AttachRefused,
  ProtocolError,
  SessionError,
  type SessionViolation,
} from "./errors.ts";
import { endViolation } from "./violation.ts";
import { FORM_BLOCK } from "./blocks.ts";
import {
  Calls,
  checkBound,
  type Unused,
  UPLOAD_STREAM,
  type UploadSend,
} from "./calls.ts";
import { type BlockFrame, fixedLimit, Reassembly } from "./reassembly.ts";

/**
 * Re-exported from `@driftsys/ridl-runtime`: a locally detected malformed
 * signal payload or payload rule violation, the check of a signal and the
 * entry that registers one.
 */
export type { Detection, SignalCheck, SignalCheckEntry };

/**
 * The state of a signal or a `fixed`, as one `publish` carried it: bytes, not
 * a decoded value. An alias of `RawSample` of `@driftsys/ridl-runtime`.
 *
 * When `detected` is set, the session itself invalidated the sample: the
 * `publish` carried a `live` payload longer than the member's limit or refused
 * by its check. The provenance is then `invalid` and `payload` is `null`; the
 * envelope is the frame's. A check that throws counts as `corrupt`.
 */
export type Sample = RawSample;

/**
 * One occurrence of a subscribed event, as bytes. An alias of `RawOccurrence`
 * of `@driftsys/ridl-runtime`.
 *
 * When `detected` is set, the session refused the value because the
 * `occurrence` carried a payload longer than the member's limit; the
 * occurrence is delivered with an empty payload, so that nothing fails
 * silently.
 */
export type Occurrence = RawOccurrence;

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
 * reads it answers resolve with an invalid sample, and the note is recorded
 * whether or not a read waited. A `check-failed` note reports a thrown signal
 * check; it does not end the session.
 */
export type Note =
  | {
    note: "check-failed";
    iface: number;
    ord: number;
    error: unknown;
  }
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

/** Re-exported from `@driftsys/ridl-runtime`: optional limits on waiting for an answer. */
export type { WaitOptions };

/** The options of {@link connect}. */
export interface ConnectOptions extends WaitOptions {
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
  /** Per-signal payload checks; a later entry replaces an earlier one. */
  checks?: readonly SignalCheckEntry[];
  /** Called for every accepted `publish` of a subscribed signal. */
  onPublish?: (target: Target, sample: Sample) => void;
  /** Called for every accepted occurrence, in arrival order. */
  onOccurrence?: (occurrence: Occurrence) => void;
  /** Called for every violation, discarded frame, loss and throwing check. */
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
 * offering the subprotocols `ridl-ws-v3`, `ridl-ws-v2` and `ridl-ws-v1`, in
 * that order, sends the `attach` and resolves once the provider answers
 * `attached(accepted)`. The `attach` carries the frame version of the
 * subprotocol the server selected: 3 on `ridl-ws-v3`, 2 on `ridl-ws-v2`, 1 on
 * `ridl-ws-v1`. On a session of frame version 1, a message of the `block`
 * form is a violation `unknown-code`, as an unknown form; on a session of
 * frame version 1 or 2, so is a message of the `chunk`, `stream_end` or
 * `stream_credit` form.
 *
 * `bound` limits the wait from this call until `attached(accepted)`, in
 * milliseconds. `signal` aborts the wait. Either closes the socket and makes
 * late open, attached and close events have no effect on the result. With
 * neither option, the wait lasts until the answer or the session ends.
 *
 * The promise rejects with:
 * - a `RangeError` for an invalid `bound` or check entry's interface number
 *   or ordinal, before any socket is created;
 * - `signal.reason` when the signal aborts, before any socket is created if
 *   it is already aborted;
 * - a {@link SessionError} with category `timeout` when the bound passes;
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
  const started = performance.now();
  let first: ReturnType<typeof attach>;
  try {
    checkWait(options);
    for (const entry of options.checks ?? []) {
      checkNumbers(entry.iface, entry.ord);
    }
    // `attach` builds an `attach` frame; its frame version is replaced by the
    // one of the selected subprotocol when the connection opens.
    first = attach(
      FRAME_VERSION,
      encodingTag(options.encoding ?? "proto3"),
      // A copy, so that the frame and `catalogHash` hold the same bytes even
      // if the caller changes its buffer before the connection opens.
      new Uint8Array(options.catalog.hash),
      options.catalog.name,
    );
  } catch (e) {
    return Promise.reject(e);
  }
  const create = options.webSocket ??
    ((u: string, p: string[]) => new WebSocket(u, p));
  let socket: WebSocket;
  try {
    socket = create(url, [SUBPROTOCOL_V3, SUBPROTOCOL, SUBPROTOCOL_V1]);
  } catch (e) {
    // For example a `SyntaxError` for an invalid URL.
    return Promise.reject(e);
  }
  return new Promise((resolve, reject) => {
    new Session(socket, first, options, { resolve, reject }, started);
  });
}

type Waiting =
  & { readonly settled: boolean }
  & (
    | {
      op: "subscribe";
      /** Set when unsubscribed or cancelled before this subscribe's answer. */
      dropped: boolean;
      resolve: () => void;
      reject: (e: unknown) => void;
    }
    | {
      op: "read";
      resolve: (sample: Sample) => void;
      reject: (e: unknown) => void;
    }
  );

/** Validates before a socket is created or a frame is sent. */
function checkWait(options: WaitOptions): void {
  if (options.bound !== undefined) checkBound(options.bound);
  options.signal?.throwIfAborted();
}

/** Settles once and clears the timer and abort listener for every outcome. */
function waitFor<T>(
  options: WaitOptions,
  resolve: (value: T) => void,
  reject: (error: unknown) => void,
  cancelled: () => void,
  started: number = performance.now(),
) {
  const signal = options.signal;
  let settled = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const cleanup = () => {
    if (settled) return false;
    settled = true;
    clearTimeout(timer);
    signal?.removeEventListener("abort", abort);
    return true;
  };
  const cancel = (error: unknown) => {
    if (!cleanup()) return;
    reject(error);
    cancelled();
  };
  const abort = () => cancel(signal?.reason);
  if (options.bound !== undefined) {
    timer = setTimeout(
      () => cancel(new SessionError({ category: "timeout" })),
      Math.max(0, options.bound - (performance.now() - started)),
    );
  }
  signal?.addEventListener("abort", abort, { once: true });
  const waiting = {
    get settled() {
      return settled;
    },
    resolve: (value: T) => {
      if (cleanup()) resolve(value);
    },
    reject: (error: unknown) => {
      if (cleanup()) reject(error);
    },
  };
  // A socket factory may have aborted the signal since connect's preflight.
  if (signal?.aborted) abort();
  return waiting;
}

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

/** The kind of each member the generated tables hold, keyed `iface:ord`. */
function recordedKinds(
  tables: readonly InterfaceTable[] | undefined,
): Map<string, Kind> {
  const kinds = new Map<string, Kind>();
  for (const table of tables ?? []) {
    for (const entry of Object.values(table)) {
      if (typeof entry !== "object") continue;
      kinds.set(channel(entry.iface, entry.ord), KIND_CODES[entry.kind]);
    }
  }
  return kinds;
}

/**
 * Adds a listener to the set under `key` and returns the function that
 * removes it.
 */
function addListener<K, L>(
  listeners: Map<K, Set<Registration<L>>>,
  key: K,
  listener: L,
): () => void {
  let set = listeners.get(key);
  if (!set) listeners.set(key, set = new Set());
  // Each call makes its own registration, so the same function registered
  // twice is called twice, and a remover removes only its own registration.
  const registration = { listener };
  set.add(registration);
  return () => {
    const current = listeners.get(key);
    if (!current?.delete(registration)) return;
    if (current.size === 0) listeners.delete(key);
  };
}

/** One registration of a listener. */
interface Registration<L> {
  readonly listener: L;
}

/**
 * Rejects with the signal's reason when the signal aborts before `call`
 * settles. The call itself stays in flight: its answer, or its bound, still
 * settles it, and nothing is left unhandled.
 */
function rejectOnAbort<T>(
  call: Promise<T>,
  signal: AbortSignal | undefined,
): Promise<T> {
  if (!signal) return call;
  return new Promise<T>((resolve, reject) => {
    const abort = () => reject(signal.reason);
    signal.addEventListener("abort", abort, { once: true });
    const done = () => signal.removeEventListener("abort", abort);
    call.then((value) => {
      done();
      resolve(value);
    }, (error) => {
      done();
      reject(error);
    });
  });
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
  #opening: Opening | null = null;
  #attachFrame: ReturnType<typeof attach>;
  /**
   * The frame version of the session: 3 on `ridl-ws-v3`, 2 on `ridl-ws-v2`, 1
   * on `ridl-ws-v1`.
   * It starts as the version of this binding and is set from the selected
   * subprotocol when the connection opens.
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
  /** The caller's payload check for each signal, keyed `iface:ord`. */
  #checks = new Map<string, SignalCheck>();
  /** The subscribed events, keyed `iface:ord`. */
  #events = new Set<string>();
  /** The last accepted `seq` of each subscribed event. */
  #lastSeq = new Map<string, bigint>();
  /** The commands and queries in flight. */
  #calls = new Calls();
  /** The limits the generated tables record, keyed by member identity. */
  #limits: Map<string, number>;
  /** The kind of each member the generated tables hold, keyed `iface:ord`. */
  #kinds: Map<string, Kind>;
  /** A copy of the catalog hash, taken when the session was created. */
  #catalogHash: Uint8Array;
  /** The `watch` listeners of each signal, keyed `iface:ord`. */
  #watchers = new Map<string, Set<Registration<(sample: Sample) => void>>>();
  /** The `listen` listeners of each interface, keyed by interface number. */
  #listeners = new Map<
    number,
    Set<Registration<(occurrence: Occurrence) => void>>
  >();
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
    attachFrame: ReturnType<typeof attach>,
    options: ConnectOptions,
    opening: Opening,
    started: number = performance.now(),
  ) {
    this.#socket = socket;
    this.#attachFrame = attachFrame;
    this.#options = options;
    this.#limits = recordedLimits(options.interfaces);
    this.#kinds = recordedKinds(options.interfaces);
    this.#catalogHash = new Uint8Array(attachFrame.hash);
    for (const entry of options.checks ?? []) {
      this.#checks.set(channel(entry.iface, entry.ord), entry.check);
    }
    this.closed = new Promise((resolve) => {
      this.#resolveClosed = resolve;
    });
    socket.binaryType = "arraybuffer";
    socket.onopen = () => this.#onOpen();
    socket.onmessage = (event) => this.#onMessage(event);
    socket.onclose = (event) => this.#onEnd(event.code, event.reason);
    socket.onerror = () => this.#onEnd(1006, "");
    const waiting = waitFor(options, opening.resolve, opening.reject, () => {
      this.#opening = null;
      this.#fail();
      if (!this.#closeSent) {
        this.#closeSent = true;
        try {
          socket.close(1000);
        } catch {
          socket.close();
        }
      }
    }, started);
    this.#opening = waiting.settled ? null : waiting;
  }

  /**
   * Subscribes a signal or an event. Resolves on `answer(accepted)`; rejects
   * with a `contract` {@link SessionError} on a refusing `answer`. A target of
   * another kind is sent as given, and the provider refuses it. Rejects with a
   * `RangeError`, and sends nothing, when the interface number or the ordinal
   * is not an integer from 1 to 4294967295, or `opts.bound` is invalid.
   * An optional `opts.bound` in milliseconds rejects with `timeout` when it
   * passes; `opts.signal` rejects with its reason on abort, without sending
   * if already aborted. A late answer is consumed without subscribing.
   * Cancellation sends `unsubscribe` if there is no established subscription
   * or other live subscribe waiting. With neither option, waits until the
   * answer or the session ends.
   *
   * With `(iface, ord)` instead of a `Target`, the session takes the kind from
   * the interface tables given to {@link connect}; an `(iface, ord)` that the
   * tables do not hold rejects with a `RangeError`, and nothing is sent.
   */
  subscribe(target: Target, opts?: WaitOptions): Promise<void>;
  subscribe(iface: number, ord: number, opts?: WaitOptions): Promise<void>;
  subscribe(
    first: Target | number,
    second?: number | WaitOptions,
    third?: WaitOptions,
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      const [target, opts] = this.#split(first, second, third);
      checkNumbers(target.iface, target.ord);
      checkWait(opts);
      if (this.#ended) return reject(new SessionError(DOWN));
      const waiting = Object.assign(
        waitFor(opts, resolve, reject, () => {
          waiting.dropped = true;
          this.#dropSubscribe(target);
        }),
        { op: "subscribe" as const, dropped: false },
      );
      this.#enqueue(target, waiting);
      try {
        this.#send({ form: "subscribe", target });
      } catch (error) {
        this.#removeWaiter(target, waiting);
        waiting.reject(error);
      }
    });
  }

  /**
   * Ends a subscription and drops what the session held for it: the held
   * sample and its `lastGood` of a signal, or the last `seq` of an event.
   * The registered signal check remains. Frames of the
   * interaction that arrive later are discarded. A `subscribe` of the same
   * interaction still waiting for its answer resolves on that answer, but does
   * not make the interaction subscribed. Does nothing once the session has
   * ended, except that an interface number or an ordinal that is not an
   * integer from 1 to 4294967295 still throws.
   *
   * @throws {RangeError} when the interface number or the ordinal is not an
   * integer from 1 to 4294967295, also after the session has ended; nothing is
   * sent.
   *
   * With `(iface, ord)` instead of a `Target`, the session takes the kind from
   * the interface tables given to {@link connect}; an `(iface, ord)` that the
   * tables do not hold throws a `RangeError`, and nothing is sent.
   */
  unsubscribe(target: Target): void;
  unsubscribe(iface: number, ord: number): void;
  unsubscribe(first: Target | number, second?: number): void {
    const [target] = this.#split(first, second, undefined);
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
   * `publish` of that interaction, unless a cancelled read ahead in the queue
   * consumes that reply without settling this read; rejects with a `contract`
   * {@link SessionError} on a refusing `answer`. On a session of frame
   * version 2, the value of a `fixed` may arrive in `block` frames, which the
   * session reassembles into one sample; a `fixed` longer than its limit, in
   * one `publish` or in blocks, resolves with an invalid sample. A signal
   * refused by its check also resolves invalid, with `detected` and `lastGood`.
   * A read does not subscribe: the sample is held only when the signal is
   * already subscribed.
   * Rejects with a `RangeError`, and sends nothing, when the interface number
   * or the ordinal is not an integer from 1 to 4294967295, or `opts.bound`
   * is invalid. An optional `opts.bound` in milliseconds rejects with
   * `timeout` when it passes; `opts.signal` rejects with its reason on abort,
   * without sending if already aborted. A late answer or payload is consumed
   * without settling a later read. With neither option, waits until the
   * answer or the session ends.
   *
   * With `(iface, ord)` instead of a `Target`, the session takes the kind from
   * the interface tables given to {@link connect}; an `(iface, ord)` that the
   * tables do not hold rejects with a `RangeError`, and nothing is sent.
   */
  read(target: Target, opts?: WaitOptions): Promise<Sample>;
  read(iface: number, ord: number, opts?: WaitOptions): Promise<Sample>;
  read(
    first: Target | number,
    second?: number | WaitOptions,
    third?: WaitOptions,
  ): Promise<Sample> {
    return new Promise((resolve, reject) => {
      const [target, opts] = this.#split(first, second, third);
      checkNumbers(target.iface, target.ord);
      checkWait(opts);
      if (this.#ended) return reject(new SessionError(DOWN));
      const waiting = Object.assign(
        waitFor(opts, resolve, reject, () => {}),
        { op: "read" as const },
      );
      this.#enqueue(target, waiting);
      try {
        this.#send({ form: "read", target });
      } catch (error) {
        this.#removeWaiter(target, waiting);
        waiting.reject(error);
      }
    });
  }

  /** The held sample of a subscribed signal, or `undefined` when there is none. */
  sample(iface: number, ord: number): Sample | undefined {
    return this.#held.get(channel(iface, ord));
  }

  /**
   * Calls `listener` with every accepted `publish` of the signal, after the
   * session updated the held sample and called `onPublish`. Returns the
   * function that removes the listener; once it returned, the listener is not
   * called again. An exception the listener throws is reported to the
   * platform and does not stop the handling of the frame. A watch does not
   * subscribe: {@link Session.subscribe} does.
   *
   * @throws {RangeError} when either number is not an integer from 1 to
   * 4294967295.
   */
  watch(
    iface: number,
    ord: number,
    listener: (sample: Sample) => void,
  ): () => void {
    checkNumbers(iface, ord);
    return addListener(this.#watchers, channel(iface, ord), listener);
  }

  /**
   * Calls `listener` with every accepted occurrence of every subscribed event
   * of the interface, in arrival order, after the session called
   * `onOccurrence`. Returns the function that removes the listener; once it
   * returned, the listener is not called again. An exception the listener
   * throws is reported to the platform and does not stop the handling of the
   * frame.
   *
   * @throws {RangeError} when `iface` is not an integer from 1 to 4294967295.
   */
  listen(
    iface: number,
    listener: (occurrence: Occurrence) => void,
  ): () => void {
    checkNumber("interface number", iface);
    return addListener(this.#listeners, iface, listener);
  }

  /**
   * The 32-byte hash of the catalog this session is attached to: the hash
   * given to {@link connect}, as it was when the session was created. Each
   * read returns a new copy.
   */
  get catalogHash(): Uint8Array {
    return new Uint8Array(this.#catalogHash);
  }

  /**
   * Reads the target of `subscribe`, `unsubscribe` and `read`: a `Target`
   * as it is, or `(iface, ord)` with the kind the interface tables hold.
   */
  #split(
    first: Target | number,
    second: number | WaitOptions | undefined,
    third: WaitOptions | undefined,
  ): [Target, WaitOptions] {
    if (typeof first !== "number") {
      return [first, (second as WaitOptions | undefined) ?? {}];
    }
    checkNumbers(first, second as number);
    const kind = this.#kinds.get(channel(first, second as number));
    if (kind === undefined) {
      throw new RangeError(
        `the interface tables hold no member ${first}:${second}`,
      );
    }
    return [{ kind, iface: first, ord: second as number }, third ?? {}];
  }

  /**
   * Registers or replaces a signal check and immediately checks a held live
   * payload. Refusal clears its bytes and `lastGood`, keeping its envelope.
   * This calls no `onPublish` and changes no sequence. A thrown check counts
   * as `corrupt` and records `check-failed`.
   * Does nothing after the session ends, except for number validation.
   *
   * @throws {RangeError} when either number is not an integer from 1 to
   * 4294967295, also after the session has ended.
   */
  checkWith(iface: number, ord: number, check: SignalCheck): void {
    checkNumbers(iface, ord);
    if (this.#ended) return;
    const ch = channel(iface, ord);
    this.#checks.set(ch, check);
    const held = this.#held.get(ch);
    if (!this.#signals.has(ch) || held?.provenance !== "live") return;
    const { detected, failure } = this.#checkSignal(iface, ord, held.payload!);
    // The check is caller code: it may have unsubscribed or closed.
    if (detected !== undefined && !this.#ended && this.#signals.has(ch)) {
      this.#held.set(ch, {
        provenance: "invalid",
        envelope: held.envelope,
        payload: null,
        lastGood: null,
        detected,
      });
    }
    if (failure) this.#note(failure);
  }

  #checkSignal(
    iface: number,
    ord: number,
    bytes: Uint8Array,
  ): { detected: Detection | undefined; failure?: Note } {
    try {
      return { detected: this.#checks.get(channel(iface, ord))?.(bytes) };
    } catch (error) {
      // Notify only after the caller commits its state: a listener may close
      // the session or unsubscribe, which must not be undone by a later write.
      return {
        detected: "corrupt",
        failure: { note: "check-failed", iface, ord, error },
      };
    }
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
   *
   * `opts.signal` aborts the call. An already aborted signal rejects with its
   * reason and sends nothing. An abort after the send rejects with the
   * signal's reason at once; the call stays pending in the session until its
   * response or its bound, and a late answer is consumed. A provider that
   * received the command may still run it.
   */
  command(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): Promise<void> {
    try {
      checkNumbers(iface, ord);
      this.#checkArgs({ kind: Kind.Command, iface, ord }, args);
      checkBound(opts.bound);
      opts.signal?.throwIfAborted();
    } catch (e) {
      return Promise.reject(e);
    }
    return rejectOnAbort(
      this.#calls.command(
        opts.bound,
        (seq) => this.#request(Kind.Command, iface, ord, seq, args),
        { kind: Kind.Command, iface, ord },
      ),
      opts.signal,
    );
  }

  /**
   * Sends a query, as {@link Session.command} sends a command. Resolves with
   * the payload of the `reply`, an empty `Uint8Array` when the reply has no
   * bytes. Rejects as a command does, including the `RangeError` for a bound
   * that is not greater than 0 and at most 2147483647 milliseconds, except
   * that a passed bound rejects with `timeout`. `opts.signal` aborts the call
   * as it aborts a command.
   */
  query(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): Promise<Uint8Array> {
    const target: Target = { kind: Kind.Query, iface, ord };
    try {
      checkNumbers(iface, ord);
      this.#checkArgs(target, args);
      checkBound(opts.bound);
      opts.signal?.throwIfAborted();
    } catch (e) {
      return Promise.reject(e);
    }
    return rejectOnAbort(
      this.#calls.query(
        opts.bound,
        this.#limitOf(target),
        (seq) => this.#request(Kind.Query, iface, ord, seq, args),
        target,
      ),
      opts.signal,
    );
  }

  /**
   * Sends a query whose reply is a stream, and returns the reply as a stream
   * of chunks, each the bytes of one `chunk` frame in order. The `request` is
   * sent as {@link Session.query} sends it. The stream holds up to 4 chunks
   * the reader has not read, the initial credit; as the reader reads, the
   * session grants the server credit for as many more (`stream_credit`).
   * Cancelling the stream sends `stream_end interrupted` and forgets the
   * call.
   *
   * The stream closes when the server ends the reply (`stream_end end`), and
   * errors with a {@link SessionError}: `contract`, `corrupt`, `busy` or
   * `interrupted` as the server's `stream_end` reports; `interrupted` also
   * when a `seq` is missing, and `corrupt` when a chunk is longer than the
   * member's limit, the client then sending `stream_end` with that outcome;
   * `timeout` when neither a `chunk` nor a `stream_end` arrives within
   * `opts.bound` milliseconds of the `request` (the timer stops at the first
   * frame of the stream), the client then sending `stream_end interrupted`; `down`
   * when the session has ended or ends first; `unsupported`, with nothing
   * sent, on a session of frame version 1 or 2, which has no stream forms;
   * `corrupt`, with nothing sent, when `args` is longer than the member's
   * limit. It errors with a `RangeError`, and sends nothing, for a bound or
   * numbers {@link Session.command} refuses. A refusing `response` to the
   * call (`contract`, `corrupt` or `busy`) before its first chunk, which a
   * server that does not serve the member sends, errors the stream with that
   * category and leaves the session open. A `response` after a chunk, a
   * `response` with `reply` or `accepted`, a `stream_credit`
   * for the call, a chunk beyond the granted credit, a stream frame
   * on a stream other than 0 ends the session with `unexpected-form`. A
   * stream frame whose target differs from its call is discarded and recorded.
   * A stream frame with a matching target for a plain command or query, with
   * neither an upload nor a streamed reply, also ends the session.
   */
  queryStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: { bound: number },
  ): ReadableStream<Uint8Array> {
    const target: Target = { kind: Kind.Query, iface, ord };
    try {
      checkNumbers(iface, ord);
      if (!this.#ended && this.#frameVersion < FRAME_VERSION_3) {
        throw new SessionError({ category: "unsupported" });
      }
      this.#checkArgs(target, args);
    } catch (e) {
      return new ReadableStream({
        start: (controller) => controller.error(e),
      });
    }
    return this.#calls.stream(opts.bound, this.#limitOf(target), {
      request: (seq) => this.#request(Kind.Query, iface, ord, seq, args),
      credit: (seq, credit) =>
        this.#send({
          form: "stream_credit",
          target,
          correlation: seq,
          stream: 0,
          credit,
        }),
      stop: (seq, outcome) => this.#stop(target, seq, outcome),
    }, target);
  }

  /**
   * Sends a command with parameter stream 1. One write sends one chunk and
   * waits while its credit is spent. The acknowledgment is bounded from the
   * request and leaves the upload open. Close resolves when its end is sent;
   * it cannot confirm that the provider accepted the last chunks. Abort sends
   * interrupted and leaves the acknowledgment waiting for its response.
   * Invalid arguments, bounds or numbers reject both operations with the
   * same error, as in {@link Session.command}; versions below 3 reject unsupported.
   */
  commandStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: { bound: number },
  ): { upload: WritableStream<Uint8Array>; accepted: Promise<void> } {
    const target = { kind: Kind.Command, iface, ord };
    try {
      const send = this.#uploadSend(target, args);
      return this.#calls.commandUpload(
        opts.bound,
        this.#limitOf(target),
        send,
        target,
      );
    } catch (error) {
      return { upload: failedUpload(error), accepted: Promise.reject(error) };
    }
  }

  /**
   * Sends a query with parameter stream 1. The reply bound starts when close
   * sends the upload's end. Abort rejects the reply with interrupted. Upload
   * failures reject the reply and every waiting write with the same error.
   * A plain reply while the upload is open errors the upload with interrupted
   * and resolves the reply. Writes and close behave as in
   * {@link Session.commandStream}.
   */
  queryWithUpload(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: { bound: number },
  ): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> } {
    const target = { kind: Kind.Query, iface, ord };
    try {
      const send = this.#uploadSend(target, args);
      return this.#calls.queryUpload(
        opts.bound,
        this.#limitOf(target),
        send,
        target,
      );
    } catch (error) {
      return { upload: failedUpload(error), reply: Promise.reject(error) };
    }
  }

  /**
   * Opens parameter stream 1 and reply stream 0 under one query. Both directions
   * have independent credit. Upload close leaves the reply open; reply end
   * interrupts an upload still open. Reply cancel sends stream-0 interrupted;
   * an abort of an open upload sends stream-1 interrupted and then stream-0
   * interrupted (an abort after close sends nothing).
   * Both forget the call and reject writes waiting for credit. Upload abort
   * resolves once both interruptions are sent. Refusal errors both
   * streams with the same error. The first-reply-frame bound starts at the
   * request, as in {@link Session.queryStream}. Preflight failures error both
   * streams and send nothing, as in {@link Session.queryWithUpload}.
   */
  queryDuplex(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: { bound: number },
  ): {
    upload: WritableStream<Uint8Array>;
    stream: ReadableStream<Uint8Array>;
  } {
    const target = { kind: Kind.Query, iface, ord };
    try {
      const send = this.#uploadSend(target, args);
      return this.#calls.duplex(opts.bound, this.#limitOf(target), {
        ...send,
        credit: (seq, credit) =>
          this.#send({
            form: "stream_credit",
            target,
            correlation: seq,
            stream: 0,
            credit,
          }),
        stop: (seq, outcome) => this.#stop(target, seq, outcome),
      }, target);
    } catch (error) {
      return {
        upload: failedUpload(error),
        stream: new ReadableStream({
          start: (controller) => controller.error(error),
        }),
      };
    }
  }

  #uploadSend(
    target: Target & { kind: typeof Kind.Command | typeof Kind.Query },
    args: Uint8Array,
  ): UploadSend {
    checkNumbers(target.iface, target.ord);
    if (!this.#ended && this.#frameVersion < FRAME_VERSION_3) {
      throw new SessionError({ category: "unsupported" });
    }
    this.#checkArgs(target, args);
    const envelope = (seq: bigint) => ({
      stamp: (this.#options.clock ?? (() => BigInt(Date.now()) * 1000n))(),
      seq,
    });
    return {
      request: (seq) =>
        this.#request(
          target.kind,
          target.iface,
          target.ord,
          seq,
          args,
        ),
      chunk: (seq, streamSeq, payload) =>
        this.#send({
          form: "chunk",
          target,
          envelope: envelope(streamSeq),
          correlation: seq,
          stream: UPLOAD_STREAM,
          payload,
        }),
      end: (seq, streamSeq, outcome) =>
        this.#send({
          form: "stream_end",
          target,
          envelope: envelope(streamSeq),
          correlation: seq,
          stream: UPLOAD_STREAM,
          outcome,
        }),
    };
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
    // `encode` copies `args` into the message it allocates. The upload's
    // send callbacks may retain the original arguments.
    this.#send({
      form: "request",
      target: { kind, iface, ord },
      envelope: { stamp: clock(), seq },
      payload: args,
    });
  }

  /**
   * Sends the `stream_end` with which the client stops the reply stream of
   * call `seq`. The client sends no chunk on that stream, so the frame takes
   * the first `seq` of its direction, 1.
   */
  #stop(target: Target, seq: bigint, outcome: StreamOutcome): void {
    const clock = this.#options.clock ?? (() => BigInt(Date.now()) * 1000n);
    this.#send({
      form: "stream_end",
      target,
      envelope: { stamp: clock(), seq: 1n },
      correlation: seq,
      stream: 0,
      outcome,
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

  #removeWaiter(target: Target, waiting: Waiting): void {
    const key = identity(target);
    const queue = this.#waiting.get(key);
    if (!queue) return;
    const index = queue.indexOf(waiting);
    if (index !== -1) queue.splice(index, 1);
    if (queue.length === 0) this.#waiting.delete(key);
  }

  /** Stops publishing when cancellation leaves no subscription or live waiter. */
  #dropSubscribe(target: Target): void {
    const ch = channel(target.iface, target.ord);
    if (
      this.#ended ||
      (target.kind === Kind.Signal && this.#signals.has(ch)) ||
      (target.kind === Kind.Event && this.#events.has(ch)) ||
      this.#waiting.get(identity(target))?.some((w) =>
        w.op === "subscribe" && !w.settled && !w.dropped
      )
    ) return;
    this.#send({ form: "unsubscribe", target });
  }

  #onOpen(): void {
    if (this.#ended) return;
    switch (this.#socket.protocol) {
      case SUBPROTOCOL_V3:
        this.#frameVersion = FRAME_VERSION_3;
        break;
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
    // Frame versions 1 and 2 have no stream forms, in the same way.
    if (
      this.#frameVersion < FRAME_VERSION_3 &&
      (message[0] === FORM_CHUNK || message[0] === FORM_STREAM_END ||
        message[0] === FORM_STREAM_CREDIT)
    ) {
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
      case "response":
        this.#unused(this.#calls.settle(frame), frame);
        return;
      case "chunk":
      case "stream_end":
      case "stream_credit":
        this.#unused(this.#calls.onStream(frame), frame);
        return;
      default:
        // A second `attached`, or a form only a consumer sends.
        this.#violate("unexpected-form");
    }
  }

  /** Records why the calls did not use a frame: a discard, or a violation. */
  #unused(reason: Unused | null, frame: Frame): void {
    if (reason === "unexpected-form") this.#violate(reason);
    else if (reason) this.#note({ note: "discarded", reason, frame });
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
   * consecutive live reads, or one settled placeholder, at the queue's head.
   */
  #onPublish(
    frame: Extract<Frame, { form: "publish" }>,
    reads: Extract<Waiting, { op: "read" }>[] = this.#takeReads(frame.target),
  ): void {
    const t = frame.target;
    const ch = channel(t.iface, t.ord);
    const subscribed = t.kind === Kind.Signal && this.#signals.has(ch);
    const held = subscribed ? this.#held.get(ch) : undefined;
    // Size refusal resolves reads with an invalid sample, retaining only the
    // last good bytes held for a subscribed signal.
    const oversized = frame.state.provenance === "live" &&
      frame.state.payload.length > this.#limitOf(t);
    // The server reuses a session `seq` only for an identical sample (the
    // crate's D-24), except `Init`, which always carries `seq` 0. Equal and
    // older sequences cannot replace the held sample. A frame answering an
    // unsettled read still needs a check before the read receives its sample.
    const dropped = !subscribed ||
      (held !== undefined && frame.envelope.seq <= held.envelope.seq);
    let sample: Sample | undefined;
    let failure: Note | undefined;
    if (!dropped || reads.some((read) => !read.settled)) {
      let detected: Detection | undefined = oversized ? "corrupt" : undefined;
      if (
        !oversized && t.kind === Kind.Signal &&
        frame.state.provenance === "live"
      ) {
        ({ detected, failure } = this.#checkSignal(
          t.iface,
          t.ord,
          frame.state.payload,
        ));
      }
      const payload =
        detected === undefined && frame.state.provenance === "live"
          ? frame.state.payload
          : null;
      sample = {
        provenance: detected === undefined ? frame.state.provenance : "invalid",
        envelope: frame.envelope,
        payload,
        lastGood: frame.state.provenance === "init"
          ? null
          : payload ?? held?.lastGood ?? null,
        ...(detected === undefined ? {} : { detected }),
      };
      // The check is caller code: when it closed the session, the reads that
      // were already taken from the queue still end with `down`.
      for (const read of reads) {
        if (this.#ended) read.reject(new SessionError(DOWN));
        else read.resolve(sample);
      }
    }

    if (!subscribed) {
      if (oversized && t.kind === Kind.Fixed) {
        this.#note({ note: "discarded", reason: "over-limit", frame });
      } else if (reads.length === 0) {
        this.#note({ note: "discarded", reason: "not-subscribed", frame });
      }
    } else if (dropped) {
      // A frame that answered a read is not a discard, though it changes nothing.
      if (
        held && frame.envelope.seq < held.envelope.seq && reads.length === 0
      ) {
        this.#note({ note: "discarded", reason: "stale", frame });
      }
    } else if (this.#ended || !this.#signals.has(ch)) {
      // The check ran caller code that ended the session or unsubscribed.
    } else {
      this.#held.set(ch, sample!);
      const onPublish = this.#options.onPublish;
      if (onPublish) this.#listen(() => onPublish(t, sample!));
      const watchers = this.#watchers.get(ch);
      if (watchers) {
        for (const watcher of [...watchers]) {
          if (watchers.has(watcher)) {
            this.#listen(() => watcher.listener(sample!));
          }
        }
      }
    }
    if (failure) this.#note(failure);
  }

  /**
   * Takes consecutive live reads, or one settled placeholder, from the head.
   * A cancelled read consumes its own reply without settling later reads.
   */
  #takeReads(t: Target): Extract<Waiting, { op: "read" }>[] {
    const reads: Extract<Waiting, { op: "read" }>[] = [];
    const key = identity(t);
    const queue = this.#waiting.get(key);
    while (queue && queue[0]?.op === "read") {
      if (reads.length > 0 && queue[0].settled) break;
      const read = queue.shift();
      if (read?.op === "read") {
        reads.push(read);
        if (read.settled) break;
      }
    }
    if (queue && queue.length === 0) this.#waiting.delete(key);
    return reads;
  }

  /**
   * Applies one `block`. A completed payload is handled as a `live`
   * `publish` that arrived whole, and settles the reads that were at the
   * head of the identity's queue when its first block arrived: a read sent
   * later waits for the next payload. A payload whose `total` is above the
   * member's limit resolves the reads at the head of the identity's queue
   * with an invalid sample (`detected: "corrupt"`, `lastGood: null`) using
   * its first block's envelope, is recorded `over-limit`, and its other
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
          r.resolve({
            provenance: "invalid",
            envelope: frame.envelope,
            payload: null,
            lastGood: null,
            detected: "corrupt",
          });
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
    const listeners = this.#listeners.get(t.iface);
    if (onOccurrence || listeners) {
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
      if (onOccurrence) this.#listen(() => onOccurrence(occurrence));
      if (listeners) {
        for (const listener of [...listeners]) {
          if (listeners.has(listener)) {
            this.#listen(() => listener.listener(occurrence));
          }
        }
      }
    }
  }

  /**
   * Ends the session for a violation: records it, closes without a code (the
   * platform cannot send 1002) and rejects everything waiting.
   */
  #violate(violation: SessionViolation): void {
    this.#violation = violation;
    const socket = this.#closeSent ? null : this.#socket;
    this.#closeSent = true;
    endViolation(socket, violation, () => {
      const opening = this.#opening;
      this.#opening = null;
      opening?.reject(new ProtocolError(violation));
      this.#fail();
    }, this.#options.onNote);
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

/** An upload refused before its request can be sent. */
function failedUpload(error: unknown): WritableStream<Uint8Array> {
  return new WritableStream({ start: (controller) => controller.error(error) });
}
