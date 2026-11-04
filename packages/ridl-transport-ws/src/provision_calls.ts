/** Request dispatch and bounded replay for the providing session. */
import { encode, Kind } from "./frame.ts";
import type { Frame, ResponseOutcome } from "./frame.ts";
import type { ProvidedRequest, ProvidedResult } from "./provision.ts";
import type { Note } from "./session.ts";

/** Advance a wire counter using unsigned 64-bit arithmetic. */
export function nextSequence(previous: bigint): bigint {
  return BigInt.asUintN(64, previous + 1n);
}

type Request = Extract<Frame, { form: "request" | "occurrence" }>;
const CAPACITY = 64;

/** No descriptor bounds are available: keep the last 64 settled responses. */
export class ProvidedCalls {
  #highest = 0n;
  #sequence = 0n;
  #active = new Set<bigint>();
  #kept = new Map<bigint, Uint8Array>();
  #ended = false;

  constructor(
    private send: (bytes: Uint8Array) => void,
    private clock: () => bigint,
    private note: (note: Note) => void,
  ) {}

  dispatch(
    frame: Request,
    limit: number,
    known: boolean,
    handler: (
      request: ProvidedRequest,
    ) => ProvidedResult | Promise<ProvidedResult>,
  ): void {
    const seq = frame.envelope.seq;
    if (seq <= this.#highest) {
      const kept = this.#kept.get(seq);
      if (kept) this.send(kept);
      return;
    }
    this.#highest = seq;
    const settle = (outcome: ResponseOutcome) => {
      if (this.#ended) return;
      const stamp = this.clock();
      this.#active.delete(seq);
      this.#sequence = nextSequence(this.#sequence);
      const bytes = encode({
        form: "response",
        target: frame.target,
        correlation: seq,
        envelope: { seq: this.#sequence, stamp },
        outcome,
      });
      this.#kept.set(seq, bytes);
      if (this.#kept.size > CAPACITY) {
        this.#kept.delete(this.#kept.keys().next().value!);
      }
      this.send(bytes);
    };
    if (!known) {
      settle({
        outcome: "contract",
        contract: { category: "unknown-interaction" },
      });
      return;
    }
    if (frame.payload.length > limit) {
      settle({ outcome: "corrupt" });
      return;
    }
    if (this.#active.size >= CAPACITY) {
      settle({ outcome: "busy" });
      return;
    }
    this.#active.add(seq);
    const discarded = () => {
      this.#active.delete(seq);
      if (!this.#ended) {
        this.note({ note: "discarded", reason: "unexpected-outcome", frame });
      }
    };
    // Both synchronous throws and promise rejections are observed. There is no
    // wire Down outcome: leave this request unanswered for the server's deadline.
    Promise.resolve().then(() => {
      if (this.#ended) return;
      return handler({
        target: { ...frame.target },
        envelope: { ...frame.envelope },
        correlation: seq,
        payload: frame.payload.slice(),
      });
    }).then((outcome) => {
      if (this.#ended) return;
      if (!outcome) {
        discarded();
        return;
      }
      if (
        (outcome.outcome === "accepted" &&
          frame.target.kind !== Kind.Command) ||
        (outcome.outcome === "reply" && frame.target.kind !== Kind.Query)
      ) {
        discarded();
        return;
      }
      settle(
        outcome.outcome === "reply" && outcome.payload.length > limit
          ? { outcome: "corrupt" }
          : outcome,
      );
    }).catch(discarded);
  }

  clear(): void {
    this.#ended = true;
    this.#active.clear();
    this.#kept.clear();
  }
}
