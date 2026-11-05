/**
 * The calls of one session in flight (frame specification §5.3, §5.4, §7 and
 * §11.1).
 *
 * A {@link Calls} table numbers each call with the session's caller counter,
 * which starts at 1, keeps a call until a response names it or its bound
 * passes, and rejects every pending call with `down` when the session ends.
 * An upload command is kept after its acknowledgment until the upload ends.
 * A query whose reply is a stream is kept until its reply
 * stream ends, is stopped by the client, or its bound passes before its first
 * frame. It sends nothing itself: the session encodes and sends every frame,
 * through the callbacks a call is started with.
 *
 * @module
 */

import { DuplexCall, type DuplexSend } from "./duplex.ts";
import { INITIAL_CREDIT, Kind } from "./frame.ts";
import type { Frame, StreamOutcome, Target } from "./frame.ts";
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
 * (`unknown-correlation`, a discard, except that `stream_credit` is silently
 * ignored), or a stream frame's target differs
 * from its call's target (also `unknown-correlation`, frame specification
 * §6.4), or a call whose shape the frame does
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
  & { target?: Target }
  & (
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
    | StreamCall
    | UploadCall
    | DuplexCall
  );

/** A query whose reply is a stream: the reply stream 0 of the call. */
interface StreamCall {
  kind: "stream";
  /** The payload limit of the queried member, in bytes, for each chunk. */
  limit: number;
  /** The bound of the first frame; cleared when the first frame arrives. */
  timer: ReturnType<typeof setTimeout>;
  target?: Target;
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

/** The parameter stream carried by the upload methods. */
export const UPLOAD_STREAM = 1;

/** The frames sent by a call with a parameter stream. */
export interface UploadSend {
  request(seq: bigint): void;
  chunk(seq: bigint, streamSeq: bigint, payload: Uint8Array): void;
  end(
    seq: bigint,
    streamSeq: bigint,
    outcome: { kind: "end" } | { kind: "interrupted" },
  ): void;
}

interface UploadCall {
  kind: "upload";
  query: boolean;
  limit: number;
  timer?: ReturnType<typeof setTimeout>;
  settled: boolean;
  open: boolean;
  granted: number;
  sent: number;
  expected: bigint;
  controller: WritableStreamDefaultController;
  failure?: unknown;
  error: (e: unknown) => void;
  waiting?: { resolve: () => void; reject: (e: unknown) => void };
  resolve: (value?: Uint8Array) => void;
  reject: (e: unknown) => void;
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
  command(
    bound: number,
    send: (seq: bigint) => void,
    target?: Target,
  ): Promise<void> {
    return new Promise((resolve, reject) => {
      this.#start(bound, send, reject, (timer) => ({
        kind: Kind.Command,
        target,
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
    target?: Target,
  ): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
      this.#start(bound, send, reject, (timer) => ({
        kind: Kind.Query,
        target,
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
   * `stream_end interrupted` when the reader cancels or when `bound`
   * milliseconds pass after the `request` with no `chunk` and no `stream_end`:
   * the timer stops at the first frame of the stream, so a stream that has
   * begun is not bounded by it. The stream holds at most
   * {@link INITIAL_CREDIT} chunks, the initial credit. It closes on
   * `stream_end end` and errors with a {@link SessionError}: the outcome of
   * any other `stream_end`, `interrupted` for a gap in `seq`, `corrupt` for a
   * chunk longer than `limit` (also sent to the server as `stream_end
   * corrupt`), `timeout` when neither a `chunk` nor a `stream_end` arrives
   * within `bound` milliseconds of the `request`, `down` when the session
   * ends.
   * It errors with a `RangeError`, and sends nothing, for a bound that is not
   * a finite number greater than 0 and at most 2147483647.
   */
  stream(
    bound: number,
    limit: number,
    send: StreamSend,
    target?: Target,
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
              target,
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

  /** Starts one query owning parameter stream 1 and reply stream 0. */
  duplex(
    bound: number,
    limit: number,
    send: DuplexSend,
    target?: Target,
  ): {
    upload: WritableStream<Uint8Array>;
    stream: ReadableStream<Uint8Array>;
  } {
    if (!Number.isFinite(bound) || bound <= 0 || bound > MAX_BOUND) {
      throw new RangeError(
        `a call bound must be greater than 0 and at most ${MAX_BOUND}`,
      );
    }
    if (this.#ended) throw new SessionError({ category: "down" });
    const seq = this.#next++;
    const call = new DuplexCall(
      target,
      seq,
      limit,
      send,
      () => this.#inFlight.delete(seq),
    );
    this.#inFlight.set(seq, call);
    call.start(bound);
    return { upload: call.upload, stream: call.stream };
  }

  /** Starts a command with an upload; its acknowledgment is bounded from the request. */
  commandUpload(
    bound: number,
    limit: number,
    send: UploadSend,
    target?: Target,
  ): { upload: WritableStream<Uint8Array>; accepted: Promise<void> } {
    const { upload, promise } = this.#upload(bound, limit, send, false, target);
    return { upload, accepted: promise.then(() => {}) };
  }

  /** Starts a query with an upload; its reply is bounded from the upload's close. */
  queryUpload(
    bound: number,
    limit: number,
    send: UploadSend,
    target?: Target,
  ): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> } {
    const { upload, promise } = this.#upload(bound, limit, send, true, target);
    return { upload, reply: promise.then((value) => value as Uint8Array) };
  }

  #upload(
    bound: number,
    limit: number,
    send: UploadSend,
    query: boolean,
    target?: Target,
  ) {
    let resolve!: (value?: Uint8Array) => void;
    let reject!: (e: unknown) => void;
    const promise = new Promise<Uint8Array | undefined>((yes, no) => {
      resolve = yes;
      reject = no;
    });
    let call: (UploadCall & { target?: Target }) | undefined;
    let seq = 0n;
    const startTimer = () => {
      if (!call) return;
      call.timer = setTimeout(() => {
        if (!call) return;
        this.#forget(seq, call);
        call.reject(
          new SessionError({ category: query ? "timeout" : "undelivered" }),
        );
      }, bound);
    };
    const sendFrame = (action: () => void) => {
      try {
        action();
      } catch (error) {
        if (call) {
          this.#forget(seq, call);
          call.reject(error);
        }
        throw error;
      }
    };
    const upload = new WritableStream<Uint8Array>({
      start: (controller) => {
        const fail = (error: unknown) => {
          controller.error(error);
          reject(error);
        };
        if (!Number.isFinite(bound) || bound <= 0 || bound > MAX_BOUND) {
          fail(
            new RangeError(
              `a call bound must be greater than 0 and at most ${MAX_BOUND}`,
            ),
          );
          return;
        }
        if (this.#ended) {
          fail(new SessionError({ category: "down" }));
          return;
        }
        seq = this.#next++;
        call = {
          kind: "upload",
          query,
          target,
          limit,
          settled: false,
          open: true,
          granted: INITIAL_CREDIT,
          sent: 0,
          expected: 1n,
          controller,
          resolve,
          error: (error) => {
            if (call) {
              call.failure = error;
              call.open = false;
            }
            controller.error(error);
            call?.waiting?.reject(error);
            if (call) call.waiting = undefined;
          },
          reject: (error) => {
            call?.error(error);
            reject(error);
          },
        };
        this.#inFlight.set(seq, call);
        try {
          send.request(seq);
        } catch (error) {
          this.#forget(seq, call);
          call.reject(error);
          return;
        }
        if (!query) startTimer();
        // Abort must release a write waiting for credit before the sink's abort runs.
        controller.signal.addEventListener("abort", () => {
          call?.waiting?.reject(new SessionError({ category: "interrupted" }));
          if (call) call.waiting = undefined;
        });
      },
      write: async (payload) => {
        if (!call) return;
        if (payload.length > call.limit) {
          const error = new SessionError({ category: "corrupt" });
          sendFrame(() =>
            send.end(seq, call!.expected, { kind: "interrupted" })
          );
          this.#forget(seq, call);
          call.reject(error);
          throw error;
        }
        if (call.sent === call.granted) {
          await new Promise<void>((resolve, reject) => {
            if (call) call.waiting = { resolve, reject };
          });
        }
        if (call.failure) throw call.failure;
        sendFrame(() => send.chunk(seq, call!.expected++, payload));
        call.sent += 1;
      },
      close: () => {
        if (!call) return;
        sendFrame(() => send.end(seq, call!.expected, { kind: "end" }));
        call.open = false;
        if (query) startTimer();
        else if (call.settled) this.#forget(seq, call);
      },
      abort: () => {
        if (!call) return;
        sendFrame(() => send.end(seq, call!.expected, { kind: "interrupted" }));
        call.open = false;
        if (query) {
          this.#forget(seq, call);
          call.reject(new SessionError({ category: "interrupted" }));
        } else if (call.settled) this.#forget(seq, call);
      },
    });
    return { upload, promise };
  }

  /**
   * Applies a `chunk`, a `stream_end` or a `stream_credit` from the server,
   * and returns `null`, or why the frame was not used: `unknown-correlation`
   * when it names no call in flight (except `stream_credit`, which is silently
   * ignored and returns `null`) or its target differs from the call;
   * `unexpected-form` for a form or stream the call does not support, or a
   * reply chunk beyond its granted credit. Uploads accept credit and an end
   * on stream 1; streamed replies accept chunks and an end on stream 0.
   */
  onStream(frame: StreamFrame): Unused | null {
    const call = this.#inFlight.get(frame.correlation);
    // A credit the provider sent before it received our `stream_end` can
    // arrive after the upload closed and its call was forgotten.
    if (!call) {
      return frame.form === "stream_credit" ? null : "unknown-correlation";
    }
    if (
      call.target && (frame.target.iface !== call.target.iface ||
        frame.target.ord !== call.target.ord ||
        frame.target.kind !== call.target.kind)
    ) {
      return "unknown-correlation";
    }
    if (call.kind === "duplex") return call.onStream(frame);
    if (call.kind === "upload") {
      if (frame.stream !== UPLOAD_STREAM || frame.form === "chunk") {
        return "unexpected-form";
      }
      if (frame.form === "stream_credit") {
        call.granted += frame.credit;
        if (call.sent < call.granted) {
          call.waiting?.resolve();
          call.waiting = undefined;
        }
        return null;
      }
      this.#forget(frame.correlation, call);
      call.reject(
        new SessionError(
          frame.envelope.seq !== 1n || frame.outcome.kind === "end"
            ? { category: "interrupted" }
            : streamFailure(frame.outcome),
        ),
      );
      return null;
    }
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
   * Settles the call a `response` names and returns `null`. Forgets the call
   * unless it is an accepted command whose upload is still open.
   * Returns why the response was not used, and changes nothing, when
   * `correlation` names no call in flight (`unknown-correlation`) or names a
   * call whose kind the outcome does not fit (`unexpected-outcome`):
   * `accepted` settles only a command and `reply` only a query. A `reply`
   * whose payload is longer than the limit the query was started with
   * rejects the query with `corrupt` and is not handed out. The limit is the
   * call's, not one taken from `frame.target`, for the reason given below.
   * A `response` for a query whose reply is a stream errors its stream and
   * forgets the call when the outcome is a refusal (`contract`, `corrupt`,
   * `busy`): a server that does not serve the member cannot know that its
   * reply is a stream, and answers `contract(UnknownInteraction)`. This holds
   * only before the stream's first `chunk`: once a chunk has arrived, the
   * server serves the member, and a `response` returns `unexpected-form`. So
   * does a `reply` or `accepted` at any time, because a served streamed query
   * is answered only by its stream.
   */
  settle(frame: Response): Unused | null {
    const call = this.#inFlight.get(frame.correlation);
    if (!call) return "unknown-correlation";
    const outcome = frame.outcome;
    if (call.kind === "duplex") return call.settle(frame);
    if (call.kind === "upload") {
      if (
        (outcome.outcome === "accepted" && call.query) ||
        (outcome.outcome === "reply" && !call.query)
      ) return "unexpected-outcome";
      if (outcome.outcome === "accepted") {
        clearTimeout(call.timer);
        call.settled = true;
        call.resolve();
        if (!call.open) this.#forget(frame.correlation, call);
      } else {
        this.#forget(frame.correlation, call);
        if (outcome.outcome === "reply") {
          if (outcome.payload.length > call.limit) {
            call.reject(new SessionError({ category: "corrupt" }));
          } else {
            if (call.open) {
              call.error(new SessionError({ category: "interrupted" }));
            }
            call.resolve(outcome.payload);
          }
        } else {
          call.reject(
            new SessionError(
              outcome.outcome === "contract"
                ? { category: "contract", contract: outcome.contract }
                : { category: outcome.outcome },
            ),
          );
        }
      }
      return null;
    }

    if (call.kind === "stream") {
      if (
        outcome.outcome === "accepted" || outcome.outcome === "reply" ||
        call.expected !== 1n
      ) {
        return "unexpected-form";
      }
      this.#forget(frame.correlation, call);
      call.reject(
        new SessionError(
          outcome.outcome === "contract"
            ? { category: "contract", contract: outcome.contract }
            : { category: outcome.outcome },
        ),
      );
      return null;
    }
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
