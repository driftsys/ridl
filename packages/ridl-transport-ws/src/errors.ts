/**
 * The errors a session rejects with.
 *
 * @module
 */

import { CallError } from "@driftsys/ridl-runtime";
import type { CallFailure } from "@driftsys/ridl-runtime";
import type { Reason, Violation } from "./frame.ts";
import type { Ended } from "./session.ts";

/**
 * A reason the consumer ends a session: a {@link Violation} of the codec, a
 * text message, a server that selected no subprotocol (in Deno; a browser
 * fails the handshake itself), a frame of a form the provider must not send
 * at that point, or a `block` sequence error (`block-sequence`): a missing,
 * repeated or reordered block, a block of index above 0 with no payload in
 * progress, a block of index 0 while a payload is in progress, a block whose
 * interface, ordinal, envelope or `total` differs from the payload in
 * progress, or a whole `publish` or an `answer` of the `fixed` whose blocks
 * are in progress.
 */
export type SessionViolation =
  | Violation
  | "text-message"
  | "no-subprotocol"
  | "unexpected-form"
  | "block-sequence";

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
 * cannot read, or sent a first message other than `attached`. In a browser, a
 * server that selected no subprotocol fails the handshake, so `connect`
 * rejects with a `SessionError` `down` (close 1006) instead; the
 * `no-subprotocol` cause happens in Deno.
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

export type { CallFailure };

/**
 * An operation of an open session failed. It is a {@link CallError} of
 * `@driftsys/ridl-runtime`, so a program that handles the failures of any port
 * can test `instanceof CallError` without importing this package; its
 * `failure` carries the category, which is one of the failures a
 * {@link CallFailure} lists: a contract error the provider reported, a failure
 * the provider reported for a call (`corrupt`, `busy`), or a failure the
 * client detected (`undelivered`: a command's bound passed; `timeout`: a
 * query, connect, subscribe or read bound passed; `down`: the session ended).
 */
export class SessionError extends CallError {
  constructor(failure: CallFailure) {
    super(failure);
    this.name = "SessionError";
    this.message = failure.category === "contract"
      ? `the operation failed: contract ${failure.contract.category}`
      : `the operation failed: ${failure.category}`;
  }
}
