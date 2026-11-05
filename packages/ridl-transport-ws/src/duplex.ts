/** The two stream directions owned by one duplex query. @module */
import { INITIAL_CREDIT } from "./frame.ts";
import type { Frame, StreamOutcome, Target } from "./frame.ts";
import { SessionError } from "./errors.ts";
import type { StreamSend, Unused, UploadSend } from "./calls.ts";

type StreamFrame = Extract<
  Frame,
  { form: "chunk" | "stream_end" | "stream_credit" }
>;
type Response = Extract<Frame, { form: "response" }>;

/** The upload and reply callbacks share one request and correlation. */
export type DuplexSend = UploadSend & Omit<StreamSend, "request">;

/** Internal call state; the containing Calls table checks targets first. */
export class DuplexCall {
  readonly kind = "duplex";
  readonly upload: WritableStream<Uint8Array>;
  readonly stream: ReadableStream<Uint8Array>;
  timer?: ReturnType<typeof setTimeout>;
  #reader!: ReadableStreamDefaultController<Uint8Array>;
  #writer!: WritableStreamDefaultController;
  #active = true;
  #open = true;
  #failure?: unknown;
  #granted = INITIAL_CREDIT;
  #sent = 0;
  #uploadSeq = 1n;
  #replyGranted = INITIAL_CREDIT;
  #received = 0;
  #replySeq = 1n;
  #waiting?: { resolve: () => void; reject: (error: unknown) => void };

  constructor(
    readonly target: Target | undefined,
    private readonly seq: bigint,
    private readonly limit: number,
    private readonly send: DuplexSend,
    private readonly forget: () => void,
  ) {
    this.stream = new ReadableStream<Uint8Array>({
      start: (controller) => {
        this.#reader = controller;
      },
      pull: (controller) => {
        if (!this.#active) return;
        const room = (controller.desiredSize ?? 0) -
          (this.#replyGranted - this.#received);
        if (room > 0) {
          this.#replyGranted += room;
          this.#send(() => send.credit(seq, room));
        }
      },
      cancel: () => {
        if (!this.#active) return;
        this.#send(() => send.stop(seq, { kind: "interrupted" }));
        this.reject(new SessionError({ category: "interrupted" }));
      },
    }, new CountQueuingStrategy({ highWaterMark: INITIAL_CREDIT }));
    this.upload = new WritableStream<Uint8Array>({
      start: (controller) => {
        this.#writer = controller;
        // The signal fires before sink abort, even with a credit-blocked write.
        controller.signal.addEventListener("abort", () => {
          if (!this.#active) return;
          try {
            this.#send(() =>
              send.end(seq, this.#uploadSeq, { kind: "interrupted" })
            );
            this.#send(() => send.stop(seq, { kind: "interrupted" }));
            // The platform abort errors the writer; do not error it first.
            this.#open = false;
            this.reject(new SessionError({ category: "interrupted" }));
          } catch {
            // #send has already failed both sides with the exact send error.
          }
        });
      },
      write: async (payload) => {
        if (payload.length > limit) {
          this.#send(() =>
            send.end(seq, this.#uploadSeq, { kind: "interrupted" })
          );
          this.#send(() => send.stop(seq, { kind: "interrupted" }));
          const error = new SessionError({ category: "corrupt" });
          this.reject(error);
          throw error;
        }
        if (this.#sent === this.#granted) {
          await new Promise<void>((resolve, reject) => {
            this.#waiting = { resolve, reject };
          });
        }
        if (!this.#active) throw this.#failure;
        this.#send(() => send.chunk(seq, this.#uploadSeq++, payload));
        this.#sent += 1;
      },
      close: () => {
        this.#send(() => send.end(seq, this.#uploadSeq, { kind: "end" }));
        this.#open = false;
      },
    });
  }

  /** Starts the first-reply-frame bound at the request. */
  start(bound: number): void {
    this.timer = setTimeout(() => {
      try {
        this.#send(() => this.send.stop(this.seq, { kind: "interrupted" }));
        this.reject(new SessionError({ category: "timeout" }));
      } catch {
        // #send already released the call and both streams.
      }
    }, bound);
    try {
      this.#send(() => this.send.request(this.seq));
    } catch {
      // Return the two errored streams without a detached rejected promise.
    }
  }

  /** Fails both directions with one error and releases every waiter. */
  reject(error: unknown): void {
    if (!this.#active) return;
    this.#finish();
    this.#failure = error;
    this.#reader.error(error);
    this.#errorUpload(error);
  }

  onStream(frame: StreamFrame): Unused | null {
    if (frame.stream === 1) {
      if (frame.form === "chunk") return "unexpected-form";
      if (frame.form === "stream_credit") {
        this.#granted += frame.credit;
        if (this.#sent < this.#granted) {
          this.#waiting?.resolve();
          this.#waiting = undefined;
        }
      } else {
        this.reject(
          frame.envelope.seq !== 1n || frame.outcome.kind === "end"
            ? new SessionError({ category: "interrupted" })
            : failure(frame.outcome),
        );
      }
      return null;
    }
    if (frame.stream !== 0 || frame.form === "stream_credit") {
      return "unexpected-form";
    }
    clearTimeout(this.timer);
    if (frame.envelope.seq !== this.#replySeq) {
      if (frame.form === "chunk") {
        this.#send(() => this.send.stop(this.seq, { kind: "interrupted" }));
      }
      this.reject(new SessionError({ category: "interrupted" }));
      return null;
    }
    if (frame.form === "stream_end") {
      if (frame.outcome.kind === "end") {
        this.#finish();
        this.#reader.close();
        this.#errorUpload(new SessionError({ category: "interrupted" }));
      } else this.reject(failure(frame.outcome));
      return null;
    }
    if (this.#received === this.#replyGranted) return "unexpected-form";
    if (frame.payload.length > this.limit) {
      this.#send(() => this.send.stop(this.seq, { kind: "corrupt" }));
      this.reject(new SessionError({ category: "corrupt" }));
      return null;
    }
    this.#received += 1;
    this.#replySeq += 1n;
    this.#reader.enqueue(frame.payload);
    return null;
  }

  settle(frame: Response): Unused | null {
    const outcome = frame.outcome;
    if (
      outcome.outcome === "accepted" || outcome.outcome === "reply" ||
      this.#replySeq !== 1n
    ) return "unexpected-form";
    this.reject(
      new SessionError(
        outcome.outcome === "contract"
          ? { category: "contract", contract: outcome.contract }
          : { category: outcome.outcome },
      ),
    );
    return null;
  }

  #errorUpload(error: unknown): void {
    this.#failure = error;
    if (this.#open) this.#writer.error(error);
    this.#open = false;
    this.#waiting?.reject(error);
    this.#waiting = undefined;
  }

  #finish(): void {
    this.#active = false;
    clearTimeout(this.timer);
    this.forget();
  }

  #send(action: () => void): void {
    try {
      action();
    } catch (error) {
      this.reject(error);
      throw error;
    }
  }
}

function failure(
  outcome: Exclude<StreamOutcome, { kind: "end" }>,
): SessionError {
  return new SessionError(
    outcome.kind === "contract"
      ? { category: "contract", contract: outcome.contract }
      : { category: outcome.kind },
  );
}
