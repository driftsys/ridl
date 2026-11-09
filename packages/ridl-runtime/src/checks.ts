import type { PayloadRule } from "./errors.ts";

/** A locally detected malformed signal payload or payload rule violation. */
export type Detection = "corrupt" | { rule: PayloadRule };

/** Returns `undefined` when the payload passes. */
export type SignalCheck = (bytes: Uint8Array) => Detection | undefined;

/** A payload check registered for one signal. */
export interface SignalCheckEntry {
  /** The interface number. */
  readonly iface: number;
  /** The member ordinal. */
  readonly ord: number;
  /** The check to run on a live signal payload. */
  readonly check: SignalCheck;
}
