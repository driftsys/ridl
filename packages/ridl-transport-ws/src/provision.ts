/** The providing side of the version-2 WebSocket binding. No client liveness timer. */
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
  Kind,
  SUBPROTOCOL_PROVIDER,
} from "./frame.ts";
import type {
  EncodingName,
  Envelope,
  Frame,
  PublishState,
  Reason,
  ResponseOutcome,
  Target,
} from "./frame.ts";
import { AttachRefused, ProtocolError, SessionError } from "./errors.ts";
import type { SessionViolation } from "./errors.ts";
import type { ConnectOptions, Ended, MemberEntry } from "./session.ts";
import { nextSequence, ProvidedCalls } from "./provision_calls.ts";
import { endViolation, notify } from "./violation.ts";
export type { EncodingName, ResponseOutcome } from "./frame.ts";
export type { MemberEntry } from "./session.ts";

/** A command or query received from the consumer. Payload bytes are opaque. */
export interface ProvidedRequest {
  target: Target;
  envelope: Envelope;
  correlation: bigint;
  payload: Uint8Array;
}
/** The wire outcome, including reply bytes only for a reply. */
export type ProvidedResult = ResponseOutcome;
/** Configuration of a remote provider. The encoding is always explicit. */
export interface ProvideOptions {
  catalog: ConnectOptions["catalog"];
  encoding: EncodingName;
  members: readonly MemberEntry[];
  onRequest: (
    request: ProvidedRequest,
  ) => Promise<ProvidedResult> | ProvidedResult;
  webSocket?: ConnectOptions["webSocket"];
  onNote?: ConnectOptions["onNote"];
  /** Channel creation and response stamp clock in microseconds; defaults to UTC, as in connect. */
  clock?: ConnectOptions["clock"];
}
const kinds: Record<MemberEntry["kind"], Kind> = {
  signal: Kind.Signal,
  event: Kind.Event,
  command: Kind.Command,
  query: Kind.Query,
  fixed: Kind.Fixed,
};
const key = (t: Target) => `${t.kind}:${t.iface}:${t.ord}`;
const unknown = {
  outcome: "contract",
  contract: { category: "unknown-interaction" },
} as const;
function numbers(iface: number, ord: number): void {
  for (const n of [iface, ord]) {
    if (!Number.isInteger(n) || n < 1 || n > 0xffff_ffff) {
      throw new RangeError(
        "interface and ordinal must be nonzero u32 integers",
      );
    }
  }
}
function stamp(value: bigint): void {
  if (value < -(1n << 63n) || value >= (1n << 63n)) {
    throw new RangeError("stamp must fit i64");
  }
}
/** Offer only the provider protocol and wait for attach, then send attached before resolving. */
export function provide(
  url: string,
  options: ProvideOptions,
): Promise<Provision> {
  try {
    attach(
      FRAME_VERSION,
      encodingTag(options.encoding),
      options.catalog.hash,
      options.catalog.name,
    );
    for (const m of options.members) {
      numbers(m.iface, m.ord);
      if (
        !Object.hasOwn(kinds, m.kind) ||
        (m.limit !== undefined &&
          (!Number.isSafeInteger(m.limit) || m.limit < 0))
      ) throw new RangeError("invalid member kind or payload limit");
    }
    const socket = (options.webSocket ?? ((u, p) => new WebSocket(u, p)))(url, [
      SUBPROTOCOL_PROVIDER,
    ]);
    return new Promise((resolve, reject) => {
      new Provision(socket, options, resolve, reject);
    });
  } catch (error) {
    return Promise.reject(error);
  }
}

type State = { envelope: Envelope; state: PublishState };
/** An accepted provider session. Construct with provide. */
export class Provision {
  /** Resolves on platform close or error, including after a local close. */
  readonly closed: Promise<void>;
  #resolveClosed!: () => void;
  #endedInfo: Promise<Ended>;
  #resolveEnded!: (ended: Ended) => void;
  #ended = false;
  #attached = false;
  #creationStamp = 0n;
  #resolve: ((p: Provision) => void) | null;
  #reject: ((e: unknown) => void) | null;
  #members = new Map<string, MemberEntry>();
  #subscribed = new Set<string>();
  #states = new Map<string, State>();
  #sequences = new Map<string, bigint>();
  #calls: ProvidedCalls;
  #clock: () => bigint;

  /** @internal Use provide. */
  constructor(
    private socket: WebSocket,
    private options: ProvideOptions,
    resolve: (p: Provision) => void,
    reject: (e: unknown) => void,
  ) {
    this.#resolve = resolve;
    this.#reject = reject;
    this.#endedInfo = new Promise((resolve) => {
      this.#resolveEnded = resolve;
    });
    this.closed = new Promise((r) => {
      this.#resolveClosed = r;
    });
    for (const m of options.members) {
      this.#members.set(key({ ...m, kind: kinds[m.kind] }), { ...m });
    }
    this.#clock = options.clock ?? (() => BigInt(Date.now()) * 1000n);
    this.#calls = new ProvidedCalls((b) => this.#sendBytes(b), () => {
      const s = this.#clock();
      stamp(s);
      return s;
    }, (n) => notify(options.onNote, n));
    socket.binaryType = "arraybuffer";
    socket.onopen = () => {
      if (!this.#ended && socket.protocol !== SUBPROTOCOL_PROVIDER) {
        this.#violate("no-subprotocol");
      }
    };
    socket.onmessage = (e) => this.#message(e);
    socket.onclose = (event) => this.#end(event.code, event.reason);
    socket.onerror = () => this.#end(1006, "");
  }
  /** Update a signal value; send it immediately when subscribed. */
  publish(iface: number, ord: number, payload: Uint8Array, time: bigint): void {
    this.#publication({ kind: Kind.Signal, iface, ord }, time, {
      provenance: "live",
      payload: payload.slice(),
    });
  }
  /** Update a signal to invalid; send it immediately when subscribed. */
  invalidate(iface: number, ord: number, time: bigint): void {
    this.#publication({ kind: Kind.Signal, iface, ord }, time, {
      provenance: "invalid",
    });
  }
  /** Raise an occurrence when subscribed; otherwise record it without delivery. */
  raise(iface: number, ord: number, payload: Uint8Array, time: bigint): void {
    const target = { kind: Kind.Event, iface, ord };
    this.#check(target, time, payload);
    const frame: Frame = {
      form: "occurrence",
      target,
      envelope: { stamp: time, seq: this.#next(target) },
      payload,
    };
    if (this.#subscribed.has(key(target))) this.#send(frame);
    else {notify(this.options.onNote, {
        note: "discarded",
        reason: "not-subscribed",
        frame,
      });}
  }
  /** Stop sends immediately; closed waits for the platform's closing handshake. */
  close(): void {
    if (this.#ended) return;
    this.socket.close(1000);
    this.#fail();
  }
  #sendBytes(bytes: Uint8Array): void {
    if (!this.#ended) this.socket.send(bytes as Uint8Array<ArrayBuffer>);
  }
  #send(frame: Frame): void {
    this.#sendBytes(encode(frame));
  }
  #next(t: Target): bigint {
    const k = key(t), n = nextSequence(this.#sequences.get(k) ?? 0n);
    this.#sequences.set(k, n);
    return n;
  }
  #limit(t: Target): number {
    return this.#members.get(key(t))?.limit ?? defaultCeiling(t.kind);
  }
  #check(t: Target, time: bigint, payload?: Uint8Array): void {
    numbers(t.iface, t.ord);
    stamp(time);
    if (this.#ended) throw new SessionError({ category: "down" });
    if (!this.#attached || !this.#members.has(key(t))) {
      throw new SessionError({
        category: "contract",
        contract: { category: "unknown-interaction" },
      });
    }
    if (payload && payload.length > this.#limit(t)) {
      throw new SessionError({ category: "corrupt" });
    }
  }
  #publication(target: Target, time: bigint, state: PublishState): void {
    this.#check(
      target,
      time,
      state.provenance === "live" ? state.payload : undefined,
    );
    const value = { envelope: { stamp: time, seq: this.#next(target) }, state };
    this.#states.set(key(target), value);
    if (this.#subscribed.has(key(target))) {
      this.#send({ form: "publish", target, ...value });
    }
  }
  #current(target: Target): void {
    this.#send({
      form: "publish",
      target,
      ...(this.#states.get(key(target)) ??
        {
          envelope: { stamp: this.#creationStamp, seq: 0n },
          state: { provenance: "init" },
        }),
    });
  }
  #message(event: MessageEvent): void {
    if (this.#ended) return;
    if (typeof event.data === "string") {
      this.#violate("text-message");
      return;
    }
    const bytes = new Uint8Array(event.data as ArrayBuffer);
    if ([FORM_CHUNK, FORM_STREAM_END, FORM_STREAM_CREDIT].includes(bytes[0])) {
      this.#violate("unknown-code");
      return;
    }
    const decoded = decode(bytes);
    if (!decoded.ok) {
      if (decoded.class === "violation") this.#violate(decoded.violation);
      else if (!this.#attached) this.#violate("unexpected-form");
      else {notify(this.options.onNote, {
          note: "discarded",
          reason: decoded.discard,
        });}
      return;
    }
    const f = decoded.frame;
    if (!this.#attached) {
      this.#first(f);
      return;
    }
    switch (f.form) {
      case "subscribe": {
        const valid = this.#members.has(key(f.target)) &&
          (f.target.kind === Kind.Signal || f.target.kind === Kind.Event);
        this.#send({
          form: "answer",
          target: f.target,
          outcome: valid ? { outcome: "accepted" } : unknown,
        });
        if (valid && !this.#subscribed.has(key(f.target))) {
          this.#subscribed.add(key(f.target));
          if (f.target.kind === Kind.Signal) this.#current(f.target);
        }
        return;
      }
      case "unsubscribe":
        if (!this.#members.has(key(f.target))) {
          notify(this.options.onNote, {
            note: "discarded",
            reason: "not-subscribed",
            frame: f,
          });
        }
        this.#subscribed.delete(key(f.target));
        return;
      case "read":
        if (f.target.kind === Kind.Signal && this.#members.has(key(f.target))) {
          this.#current(f.target);
        } else {this.#send({
            form: "answer",
            target: f.target,
            outcome: unknown,
          });}
        return;
      case "request":
        this.#calls.dispatch(
          f,
          this.#limit(f.target),
          this.#members.has(key(f.target)) &&
            (f.target.kind === Kind.Command || f.target.kind === Kind.Query),
          this.options.onRequest,
        );
        return;
      default:
        this.#violate("unexpected-form");
    }
  }
  #first(f: Frame): void {
    if (f.form !== "attach") {
      this.#violate("unexpected-form");
      return;
    }
    let reason: Reason | undefined;
    if (f.frameVersion !== FRAME_VERSION) reason = "frame-version-unsupported";
    else if (f.name !== this.options.catalog.name) reason = "unknown-catalog";
    else if (!f.hash.every((b, i) => b === this.options.catalog.hash[i])) {
      reason = "catalog-mismatch";
    } else if (f.encoding !== encodingTag(this.options.encoding)) {
      reason = "encoding-unsupported";
    }
    if (!reason) {
      try {
        const creation = this.#clock();
        stamp(creation);
        this.#creationStamp = creation;
      } catch (error) {
        this.#reject?.(error);
        this.#reject = null;
        this.#fail();
        this.socket.close();
        return;
      }
    }
    this.#send({
      form: "attached",
      outcome: reason
        ? { outcome: "refused", reason }
        : { outcome: "accepted" },
    });
    if (reason) {
      // AttachRefused uses the consumer's close metadata shape without changing
      // Provision.closed, whose public contract is Promise<void>.
      this.#reject?.(
        new AttachRefused(
          reason,
          this.#endedInfo.then((ended) => ({ ...ended, refused: reason })),
        ),
      );
      this.#reject = null;
      this.close();
      return;
    }
    this.#attached = true;
    this.#resolve?.(this);
    this.#resolve = null;
    this.#reject = null;
  }
  #violate(v: SessionViolation): void {
    endViolation(this.socket, v, () => {
      this.#reject?.(new ProtocolError(v));
      this.#reject = null;
      this.#fail();
    }, this.options.onNote);
  }
  #fail(): void {
    this.#ended = true;
    this.#resolve = null;
    this.#reject?.(new SessionError({ category: "down" }));
    this.#reject = null;
    this.#calls.clear();
    this.#subscribed.clear();
    this.#states.clear();
    this.#sequences.clear();
  }
  #end(code: number, reason: string): void {
    this.#resolveEnded({ code, reason });
    this.#fail();
    this.#resolveClosed();
  }
}
