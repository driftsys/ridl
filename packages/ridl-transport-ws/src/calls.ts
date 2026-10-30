/**
 * The calls of one session in flight (frame specification §5.3, §5.4 and §7).
 *
 * A {@link Calls} table numbers each call with the session's caller counter,
 * which starts at 1, keeps the call until a `response` names its number in
 * `correlation` or its bound passes, and rejects every call with `down` when
 * the session ends. It sends nothing itself: the session encodes and sends
 * the `request`.
 *
 * @module
 */

import { Kind } from "./frame.ts";
import type { Frame } from "./frame.ts";
import { SessionError } from "./errors.ts";

/** The longest bound `setTimeout` can wait, in milliseconds: 2^31 - 1. */
const MAX_BOUND = 2_147_483_647;

type Response = Extract<Frame, { form: "response" }>;

type Call =
  | {
    kind: typeof Kind.Command;
    timer: ReturnType<typeof setTimeout>;
    resolve: () => void;
    reject: (e: SessionError) => void;
  }
  | {
    kind: typeof Kind.Query;
    timer: ReturnType<typeof setTimeout>;
    resolve: (reply: Uint8Array) => void;
    reject: (e: SessionError) => void;
  };

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
   * the bound passes.
   */
  query(bound: number, send: (seq: bigint) => void): Promise<Uint8Array> {
    return new Promise((resolve, reject) => {
      this.#start(bound, send, reject, (timer) => ({
        kind: Kind.Query,
        timer,
        resolve,
        reject,
      }));
    });
  }

  /**
   * Settles the call a `response` names and forgets it, and returns `null`.
   * Returns why the response was not used, and changes nothing, when
   * `correlation` names no call in flight (`unknown-correlation`) or names a
   * call whose kind the outcome does not fit (`unexpected-outcome`):
   * `accepted` settles only a command and `reply` only a query. A `reply`
   * whose payload is longer than `limit` bytes, the member's limit, rejects
   * the query with `corrupt` and is not handed out.
   */
  settle(
    frame: Response,
    limit: number,
  ): "unknown-correlation" | "unexpected-outcome" | null {
    const call = this.#inFlight.get(frame.correlation);
    if (!call) return "unknown-correlation";
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
        if (outcome.payload.length > limit) {
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
