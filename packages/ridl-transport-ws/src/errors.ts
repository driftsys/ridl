/**
 * The errors a session rejects with.
 *
 * @module
 */

import type { Contract, Reason, Violation } from "./frame.ts";
import type { Ended } from "./session.ts";

/**
 * A reason the consumer ends a session: a {@link Violation} of the codec, a
 * text message, a server that selected no subprotocol, or a frame of a form
 * the provider must not send at that point.
 */
export type SessionViolation =
  | Violation
  | "text-message"
  | "no-subprotocol"
  | "unexpected-form";

/** The provider refused the `attach`: the session never opened. */
export class AttachRefused extends Error {
  /** The reason the provider gave in `attached(refused)`. */
  readonly reason: Reason;
  /**
   * Resolves once the connection has closed, with `refused` set to
   * {@link AttachRefused.reason}.
   */
  readonly closed: Promise<Ended>;

  constructor(reason: Reason, closed: Promise<Ended>) {
    super(`the provider refused the attach: ${reason}`);
    this.name = "AttachRefused";
    this.reason = reason;
    this.closed = closed;
  }
}

/**
 * The session ended before it opened because the server broke the binding: it
 * selected no subprotocol, sent a text message, sent a message the codec
 * cannot read, or sent a first message other than `attached`.
 */
export class ProtocolError extends Error {
  /** What the server did. */
  override readonly cause: SessionViolation;

  constructor(cause: SessionViolation) {
    super(`the server broke the WebSocket binding: ${cause}`);
    this.name = "ProtocolError";
    this.cause = cause;
  }
}

/**
 * Why an operation failed: a contract error the provider reported, a failure
 * the provider reported for a call (`corrupt`, `busy`), or a failure the
 * client detected (`undelivered`: a command's bound passed; `timeout`: a
 * query's bound passed; `down`: the session ended).
 */
export type CallFailure =
  | { category: "contract"; contract: Contract }
  | { category: "corrupt" }
  | { category: "busy" }
  | { category: "undelivered" }
  | { category: "timeout" }
  | { category: "down" };

/** An operation of an open session failed. */
export class SessionError extends Error {
  /** The failure, with its category. */
  readonly failure: CallFailure;

  constructor(failure: CallFailure) {
    super(
      failure.category === "contract"
        ? `the operation failed: contract ${failure.contract.category}`
        : `the operation failed: ${failure.category}`,
    );
    this.name = "SessionError";
    this.failure = failure;
  }
}
