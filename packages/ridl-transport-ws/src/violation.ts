/** Shared browser close and listener handling for both session roles. */
import type { SessionViolation } from "./errors.ts";
import type { Note } from "./session.ts";

export function notify(
  onNote: ((note: Note) => void) | undefined,
  note: Note,
): void {
  try {
    onNote?.(note);
  } catch (error) {
    globalThis.reportError(error);
  }
}

/** The platform accepts neither 1002 nor 1003. End operations before notifying. */
export function endViolation(
  socket: WebSocket | null,
  violation: SessionViolation,
  fail: () => void,
  onNote: ((note: Note) => void) | undefined,
): void {
  socket?.close();
  fail();
  notify(onNote, { note: "violation", violation });
}
