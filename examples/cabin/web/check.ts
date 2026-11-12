// The check of the cabin demo, without any Deno API, so that a browser page
// can load it too. It judges the sequence of received temperature values and
// the replies of the two calls; main.ts and page.ts supply the transport and
// bind the generated `CabinClient`, which verifies and decodes each payload
// with the wasm codec that they load. It builds the payloads of its self-test
// with the generated `encodeTemperature`, because the client refuses to send or
// accept a payload that the codec rejects.

import type { Codec } from "../../../packages/ridl-runtime/mod.ts";
import type { connect } from "../../../packages/ridl-transport-ws/mod.ts";
import {
  Cabin,
  type CabinClient,
  CATALOG_HASH,
  CATALOG_NAME,
  encodeTemperature,
  Horn,
  RULES,
} from "./generated/veh.cabin.codec.ts";
import {
  makeLevel,
  makeWindow,
  type Temperature,
} from "./generated/veh.cabin.ts";

/** The lowest and the highest value of `Temperature` in cabin.ridl. */
export const LOWEST = -40;
export const HIGHEST = 85;

/** The number of values in the range of `Temperature`. */
export const RING = HIGHEST - LOWEST + 1;

/**
 * The largest forward step between two received values that is not an error.
 * The server publishes forward on the ring of RING values, and the transport
 * sends only the newest value of a signal per tick, so samples between two
 * ticks are skipped and a step larger than one is normal. A forward step of s
 * and a backward step of RING - s give the same difference modulo RING, so only
 * steps up to half the ring can be told apart from a backward move: 63 is
 * RING / 2. A gap of more than 63 publications between two received values
 * therefore fails a correct server. The measured period of the server is about
 * 10 ms (the 7 ms sleep overshoots), so that is a gap of about 630 ms. A step
 * of 0 is a value sent twice, which the transport can do; it is ignored.
 *
 * Seeing 85 reached by a step, and then -40, is what rejects a server that
 * wraps to another value, a server that alternates the bounds, and a decoder
 * that is off by a constant c. The -40 of the first sample does not count,
 * because it comes before any wrap, and neither does an 85 in the first
 * sample, because no step has checked it. With c < 0 the range check fails, or
 * 85 is never seen. With c > 0 the range check fails when 85 + c is received,
 * and otherwise -40 is never seen after 85.
 */
export const MAX_STEP = RING / 2;

/** What the check has seen so far. */
export interface Progress {
  /** The number of values accepted, repeats included. */
  received: number;
  /** The number of steps from 85 to -40 or across it. */
  wraps: number;
  /** True when 85 was reached by a checked step. */
  sawHighest: boolean;
  /** True when -40 was received after 85 had been received. */
  sawLowestAfter: boolean;
}

/**
 * True when `progress` is a progress that passed the check: an object with at
 * least one received value, 85 seen, and -40 seen after 85.
 */
export function isPass(progress: unknown): progress is Progress {
  if (typeof progress !== "object" || progress === null) return false;
  const p = progress as Progress;
  return p.received > 0 && p.sawHighest === true && p.sawLowestAfter === true;
}

/** The result of accepting one value. */
export type Verdict =
  | { kind: "continue" }
  | { kind: "pass" }
  | { kind: "fail"; cause: string };

/** Converts the catalog hash, 32 bytes, to its 64 hex digits. */
export function hexFromHash(hash: Uint8Array): string {
  return Array.from(hash, (b) => b.toString(16).padStart(2, "0")).join("");
}

/**
 * The code `ridl_verify` returns for a broken range: 2 plus the index of
 * `range` in the generated `RULES`.
 */
export const RANGE_RULE = 2 + RULES.indexOf("range");

/**
 * The payload of a temperature, built with the generated `encodeTemperature`,
 * which does not check the range, so that a payload the codec rejects can be
 * built.
 */
function encode(value: number): Uint8Array {
  return encodeTemperature(value as Temperature);
}

/**
 * Checks that the codec accepts 20 and rejects -41 and 86 with `RANGE_RULE`.
 * Returns `undefined` when it does, and otherwise a cause that names the
 * value and the result. The demos run it before they verify any received
 * payload, so that a codec that accepts every payload cannot pass them.
 */
export function selfTest(codec: Codec): string | undefined {
  const id = codec.typeId(Cabin.temperature.payload.type);
  const accepted = codec.verify(id, encode(20));
  if (!accepted.ok) {
    return `the codec rejected 20, which is inside [${LOWEST}..${HIGHEST}]: ` +
      JSON.stringify(accepted);
  }
  for (const value of [LOWEST - 1, HIGHEST + 1]) {
    const result = codec.verify(id, encode(value));
    if (result.ok) {
      return `the codec accepted ${value}, which is outside ` +
        `[${LOWEST}..${HIGHEST}]`;
    }
    if (result.kind !== "rule" || result.code !== RANGE_RULE) {
      return `the codec rejected ${value} with ${JSON.stringify(result)}, ` +
        `not with rule code ${RANGE_RULE} (range)`;
    }
  }
  return undefined;
}

/** Judges the sequence of received temperature values. */
export class TemperatureCheck {
  readonly progress: Progress = {
    received: 0,
    wraps: 0,
    sawHighest: false,
    sawLowestAfter: false,
  };
  #previous: number | undefined;
  #failure: Verdict | undefined;

  /** Accepts the next received value. After a failure, every call fails. */
  accept(value: number): Verdict {
    if (this.#failure) return this.#failure;
    const progress = this.progress;
    progress.received++;
    if (value < LOWEST || value > HIGHEST) {
      return this.#fail(
        `temperature ${value} is outside [${LOWEST}..${HIGHEST}]`,
      );
    }
    const previous = this.#previous;
    if (previous !== undefined) {
      // The same value again is a repeated send, not progress.
      if (value === previous) return { kind: "continue" };
      const step = (value - previous + RING) % RING;
      if (step < 1 || step > MAX_STEP) {
        return this.#fail(
          `temperature went from ${previous} to ${value}, a step of ` +
            `${step} on the ring of ${RING} values, not 1 to ${MAX_STEP}`,
        );
      }
      if (value < previous) progress.wraps++;
      // Only a value reached by a checked step counts as seen.
      if (value === HIGHEST) progress.sawHighest = true;
      if (value === LOWEST && progress.sawHighest) {
        progress.sawLowestAfter = true;
      }
    }
    this.#previous = value;
    return progress.sawLowestAfter ? { kind: "pass" } : { kind: "continue" };
  }

  #fail(cause: string): Verdict {
    this.#failure = { kind: "fail", cause };
    return this.#failure;
  }
}

/**
 * The time `connect` and `subscribe` each wait for the server, in milliseconds.
 * The server answers within a few milliseconds; the bound makes a server that
 * never answers reject the call with `timeout`.
 */
export const STEP_BOUND_MS = 10_000;

/**
 * Subscribes `client` to the temperature signal with the bound of each step.
 * main.ts and page.ts both call it, so that one test of the bound covers both.
 */
export function subscribeTemperature(
  client: Pick<CabinClient, "subscribeTemperature">,
): Promise<void> {
  return client.subscribeTemperature({ bound: STEP_BOUND_MS });
}

/** The level that the demo sends with `setLevel`. */
export const LEVEL = 42;

/** The averaging window, in milliseconds, that the demo sends with `average`. */
export const WINDOW_MS = 10;

/**
 * Sends `setLevel` with `LEVEL` and then calls `average` with `WINDOW_MS`.
 * The server answers `average` with the mean of the levels it received, and it
 * received only this one, so the reply has to be `LEVEL`. Returns `undefined`
 * when it is, and otherwise a cause. A call that fails rejects, and the caller
 * reports the rejection. Both calls use the bound of each step, not the bound
 * the catalog declares (50 ms and 200 ms), so that a loaded machine does not
 * fail the demo on a delay.
 */
export async function exerciseCalls(
  client: Pick<CabinClient, "setLevel" | "average">,
): Promise<string | undefined> {
  const opts = { bound: STEP_BOUND_MS };
  await client.setLevel(makeLevel(LEVEL), opts);
  const average = await client.average(makeWindow(WINDOW_MS), opts);
  return average === LEVEL
    ? undefined
    : `average replied ${average} after setLevel(${LEVEL}), not ${LEVEL}`;
}

/**
 * The options that main.ts and page.ts give to `connect`: the catalog, the
 * encoding, the limits of the generated interfaces and the bound of each step.
 * Each program adds its own `onNote`. The signal checks are registered by
 * `CabinClient.bind`, not here.
 */
export function sessionOptions(): Pick<
  Parameters<typeof connect>[1],
  "catalog" | "encoding" | "interfaces" | "bound"
> {
  return {
    catalog: { name: CATALOG_NAME, hash: CATALOG_HASH },
    encoding: "flatbuffers",
    // The generated tables give the session the limit of each member that a
    // `[limits]` entry raises or lowers; cabin has none, so every member
    // keeps the default ceiling of its kind.
    interfaces: [Cabin, Horn],
    bound: STEP_BOUND_MS,
  };
}
