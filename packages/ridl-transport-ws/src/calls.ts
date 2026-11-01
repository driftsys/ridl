/**
 * The calls of one session in flight (frame specification §5.3, §5.4, §7 and
 * §11.1).
 *
 * A {@link Calls} table numbers each call with the session's caller counter,
 * which starts at 1, keeps the call until a `response` names its number in
 * `correlation` or its bound passes, and rejects every call with `down` when
 * the session ends. A query whose reply is a stream is kept until its reply
 * stream ends, is stopped by the client, or its bound passes before its first
 * frame. It sends nothing itself: the session encodes and sends every frame,
 * through the callbacks a call is started with.
 *
 * @module
 */

import { INITIAL_CREDIT, Kind } from "./frame.ts";
import type { Frame, StreamOutcome } from "./frame.ts";
import { type CallFailure, SessionError } from "./errors.ts";

/** The longest bound `setTimeout` can wait, in milliseconds: 2^31 - 1. */
const MAX_BOUND = 2_147_483_647;

type Response = Extract<Frame, { form: "response" }>;
type StreamFrame = Extract<
  Frame,
  { form: "chunk" | "stream_end" | "stream_credit" }
>;

/**
 * Why the session did not use a frame: it names no call in flight
 * (`unknown-correlation`, a discard), or a call whose shape the frame does
 * not fit (`unexpected-outcome`, a discard; `unexpected-form`, a violation
 * that ends the session).
 */
export type Unused =
  | "unknown-correlation"
  | "unexpected-outcome"
  | "unexpected-form";

/** The frames a query whose reply is a stream sends after its `request`. */
export interface StreamSend {
  /** Sends the `request`, with `seq` as its `seq`. */
  request(seq: bigint): void;
  /** Sends `stream_credit` with `credit` chunks for the reply stream of call `seq`. */
  credit(seq: bigint, credit: number): void;
  /** Sends `stream_end` with `outcome` on the reply stream of call `seq`. */
  stop(seq: bigint, outcome: StreamOutcome): void;
}

type Call =
  | {
    kind: typeof Kind.Command;
    timer: ReturnType<typeof setTimeout>;
    resolve: () => void;
    reject: (e: SessionError) => void;
  }
  | {
    kind: typeof Kind.Query;
    /** The payload limit of the queried member, in bytes. */
    limit: number;
    timer: ReturnType<typeof setTimeout>;
    resolve: (reply: Uint8Array) => void;
    reject: (e: SessionError) => void;
  }
  | StreamCall;

/** A query whose reply is a stream: the reply stream 0 of the call. */
interface StreamCall {
  kind: "stream";
  /** The payload limit of the queried member, in bytes, for each chunk. */
  limit: number;
  /** The bound of the first frame; cleared when the first frame arrives. */
  timer: ReturnType<typeof setTimeout>;
  /** The chunks the client has granted: the initial credit and every `stream_credit`. */
  granted: number;
  /** The chunks received. */
  received: number;
  /** The `seq` the next `chunk` or the `stream_end` carries. */
  expected: bigint;
  controller: ReadableStreamDefaultController<Uint8Array>;
  /** Sends `stream_end` with `outcome` for the call. */
  stop: (outcome: StreamOutcome) => void;
  reject: (e: SessionError) => void;
}

/** The calls in flight of one session, keyed by the `seq` of their `request`. */
export class Calls {
  #next = 1n;
  #ended = false;
  #inFlight = new Map<bigint, Call>();

  /**
   * Starts a command. `send` is called once, with the caller number to use as
   * the `request`'s `seq`, and must send the `request`. The promise resolves
   * on `accepted` and rejects with a {@link SessionError}: the provider's
   * `contract`, `corrupt` or `busy`; `undelivered` when `bound` milliseconds
   * pass with no `response`; `down` when the session ends. It rejects with a
   * `RangeError`, before `send` is called, when `bound` is not a finite number
   * greater than 0 and at most 2147483647.
   */
  command(bound: number, send: (seq: bigint) => void): Promise<void> {
    return new Promise((resolve, reject) => {
      this.#start(bound, send, reject, (timer) => ({
        kind: Kind.Command,
        timer,
        resolve,
        reject,
      }));
    });
  }

  /**
   * Starts a query, as {@link Calls.command} does. The promise resolves with
   * the payload of the `reply`, and rejects `timeout`, not `undelivered`, when
   * the bound passes. `limit` is the payload limit of the queried member, in
   * bytes: a `reply` whose payload is longer rejects the query with `corrupt`.
   */
  query(
    bound: number,
    limit: number,
    send: (seq: bigint) => void,
  ): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
      this.#start(bound, send, reject, (timer) => ({
        kind: Kind.Query,
        limit,
        timer,
        resolve,
        reject,
      }));
    });
  }

  /**
   * Starts a query whose reply is a stream, and returns that stream. `send`
   * sends the call's frames: the `request` at once, within this call, then
   * a `stream_credit` from the stream's `pull` each time the reader has room
   * for more chunks than are granted and not yet received, and a
   * `stream_end interrupted` when the reader cancels or `bound` milliseconds
   * pass with no `chunk` and no `stream_end`. The stream holds at most
   * {@link INITIAL_CREDIT} chunks, the initial credit. It closes on
   * `stream_end end` and errors with a {@link SessionError}: the outcome of
   * any other `stream_end`, `interrupted` for a gap in `seq`, `corrupt` for a
   * chunk longer than `limit` (also sent to the server as `stream_end
   * corrupt`), `timeout` when the bound passes, `down` when the session ends.
   * It errors with a `RangeError`, and sends nothing, for a bound that is not
   * a finite number greater than 0 and at most 2147483647.
   */
  stream(
    bound: number,
    limit: number,
    send: StreamSend,
  ): ReadableStream<Uint8Array> {
    let seq = 0n;
    let call: StreamCall | undefined;
    return new ReadableStream<Uint8Array>({
      start: (controller) => {
        this.#start(
          bound,
          (s) => {
            seq = s;
            send.request(s);
          },
          (e) => controller.error(e),
          (timer) => {
            call = {
              kind: "stream",
              limit,
              timer,
              granted: INITIAL_CREDIT,
              received: 0,
              expected: 1n,
              controller,
              stop: (outcome) => send.stop(seq, outcome),
              reject: (e) => controller.error(e),
            };
            return call;
          },
        );
      },
      pull: (controller) => {
        if (!call || this.#inFlight.get(seq) !== call) return;
        // The room the reader has, less the chunks already granted and not
        // yet received: what the server may send without overfilling it.
        const room = (controller.desiredSize ?? 0) -
          (call.granted - call.received);
        if (room > 0) {
          call.granted += room;
          send.credit(seq, room);
        }
      },
      cancel: () => {
        if (!call || this.#inFlight.get(seq) !== call) return;
        this.#forget(seq, call);
        call.stop({ kind: "interrupted" });
      },
    }, new CountQueuingStrategy({ highWaterMark: INITIAL_CREDIT }));
  }

  /**
   * Applies a `chunk`, a `stream_end` or a `stream_credit` from the server,
   * and returns `null`, or why the frame was not used: `unknown-correlation`
   * when it names no call in flight, and changes nothing;
   * `unexpected-form` when it names a call with no reply stream, a stream
   * other than 0, or is a `stream_credit` (the client sends no stream), or
   * when a `chunk` exceeds the credit granted.
   */
  onStream(frame: StreamFrame): Unused | null {
    const call = this.#inFlight.get(frame.correlation);
    if (!call) return "unknown-correlation";
    if (
      call.kind !== "stream" || frame.form === "stream_credit" ||
      frame.stream !== 0
    ) {
      return "unexpected-form";
    }
    clearTimeout(call.timer);
    const seq = frame.correlation;
    // A gap in `seq` means that frames of the stream were lost.
    if (frame.envelope.seq !== call.expected) {
      this.#forget(seq, call);
      if (frame.form === "chunk") call.stop({ kind: "interrupted" });
      call.reject(new SessionError({ category: "interrupted" }));
      return null;
    }
    if (frame.form === "stream_end") {
      this.#forget(seq, call);
      if (frame.outcome.kind === "end") call.controller.close();
      else call.reject(new SessionError(streamFailure(frame.outcome)));
      return null;
    }
    if (call.received === call.granted) return "unexpected-form";
    if (frame.payload.length > call.limit) {
      this.#forget(seq, call);
      call.stop({ kind: "corrupt" });
      call.reject(new SessionError({ category: "corrupt" }));
      return null;
    }
    call.received += 1;
    call.expected += 1n;
    call.controller.enqueue(frame.payload);
    return null;
  }

  /**
   * Settles the call a `response` names and forgets it, and returns `null`.
   * Returns why the response was not used, and changes nothing, when
   * `correlation` names no call in flight (`unknown-correlation`) or names a
   * call whose kind the outcome does not fit (`unexpected-outcome`):
   * `accepted` settles only a command and `reply` only a query. A `reply`
   * whose payload is longer than the limit the query was started with
   * rejects the query with `corrupt` and is not handed out. The limit is the
   * call's, not one taken from `frame.target`, for the reason given below.
   * A `response` for a query whose reply is a stream returns
   * `unexpected-form`: such a query is answered only by its stream.
   */
  settle(frame: Response): Unused | null {
    const call = this.#inFlight.get(frame.correlation);
    if (!call) return "unknown-correlation";
    if (call.kind === "stream") return "unexpected-form";
    const outcome = frame.outcome;
    // The outcome is compared with the call's kind, not with
    // `frame.target.kind`: §6.4 lets a contract(UnknownInteraction) response
    // echo another kind.
    if (
      (outcome.outcome === "accepted" && call.kind !== Kind.Command) ||
      (outcome.outcome === "reply" && call.kind !== Kind.Query)
    ) {
      return "unexpected-outcome";
    }
    this.#forget(frame.correlation, call);
    switch (outcome.outcome) {
      case "accepted":
        if (call.kind === Kind.Command) call.resolve();
        return null;
      case "reply":
        if (call.kind !== Kind.Query) return null; // ruled out above
        if (outcome.payload.length > call.limit) {
          call.reject(new SessionError({ category: "corrupt" }));
        } else {
          call.resolve(outcome.payload);
        }
        return null;
      case "contract":
        call.reject(
          new SessionError({
            category: "contract",
            contract: outcome.contract,
          }),
        );
        return null;
      case "corrupt":
      case "busy":
        call.reject(new SessionError({ category: outcome.outcome }));
        return null;
    }
  }

  /**
   * Ends the table: rejects every call in flight with `down`, clears its
   * timer, and makes every later call reject with `down` without sending.
   */
  fail(): void {
    this.#ended = true;
    const calls = [...this.#inFlight.values()];
    this.#inFlight.clear();
    for (const call of calls) clearTimeout(call.timer);
    for (const call of calls) {
      call.reject(new SessionError({ category: "down" }));
    }
  }

  #start(
    bound: number,
    send: (seq: bigint) => void,
    reject: (e: unknown) => void,
    make: (timer: ReturnType<typeof setTimeout>) => Call,
  ): void {
    // `setTimeout` takes a signed 32-bit delay; a longer one fires at once.
    if (!Number.isFinite(bound) || bound <= 0 || bound > MAX_BOUND) {
      reject(
        new RangeError(
          `a call bound must be a number of milliseconds greater than 0 and at most ${MAX_BOUND}, not ${bound}`,
        ),
      );
      return;
    }
    if (this.#ended) {
      reject(new SessionError({ category: "down" }));
      return;
    }
    const seq = this.#next;
    this.#next += 1n;
    send(seq);
    const timer = setTimeout(() => {
      const call = this.#inFlight.get(seq);
      if (!call) return;
      this.#forget(seq, call);
      if (call.kind === "stream") call.stop({ kind: "interrupted" });
      call.reject(
        new SessionError({
          category: call.kind === Kind.Command ? "undelivered" : "timeout",
        }),
      );
    }, bound);
    this.#inFlight.set(seq, make(timer));
  }

  #forget(seq: bigint, call: Call): void {
    clearTimeout(call.timer);
    this.#inFlight.delete(seq);
  }
}

/** The failure a `stream_end` other than `end` reports. */
function streamFailure(
  outcome: Exclude<StreamOutcome, { kind: "end" }>,
): CallFailure {
  return outcome.kind === "contract"
    ? { category: "contract", contract: outcome.contract }
    : { category: outcome.kind };
}
