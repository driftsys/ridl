// The check of the cabin demo, without any Deno API, so that a browser page
// can load it too. It decodes the payload of `Cabin.temperature` and judges
// the sequence of received values; main.ts and page.ts supply the transport.
// It also builds a payload and verifies payloads through the wasm codec that
// main.ts and page.ts load.

import * as flatbuffers from "flatbuffers";
import type { Codec } from "../../../packages/ridl-runtime/mod.ts";
import { TemperatureBox } from "./generated/veh/cabin.ts";

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

/**
 * The (interface, ordinal) pair of the interaction the check reads. The pair
 * is the one in cabin.ridl: interface 1 is `Cabin`, and its interaction 1 is
 * the signal `temperature`.
 */
export const TEMPERATURE = { iface: 1, ord: 1 };

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

/** Converts the catalog hash, 64 hex digits, to its 32 bytes. */
export function hashFromHex(hex: string): Uint8Array {
  return Uint8Array.from(hex.match(/../g)!.map((h) => parseInt(h, 16)));
}

/**
 * The `type_id` of the payload type that carries `Temperature`: its index in
 * `WASM_PAYLOAD_TYPES` of the generated
 * examples/cabin/generated/__ridl_wasm.rs, which lists the payload types
 * sorted by qualified name. The entry is `veh.cabin.Temperature`, the sixth
 * one; its payload is the `TemperatureBox` table that `decodeTemperature`
 * reads. check_test.ts reads that file and checks this index.
 */
export const TEMPERATURE_TYPE_ID = 5;

/**
 * The code `ridl_verify` returns for a broken range: 2 plus the index of
 * `range` in `WASM_RULES` (`length`, `pattern`, `range`, ...).
 */
export const RANGE_RULE = 4;

/**
 * Builds the payload of a `TemperatureBox`, the root type that
 * `decodeTemperature` reads, with the `flatc` builder. It does not check the
 * range, so that a payload the codec rejects can be built.
 */
export function encodeTemperature(value: number): Uint8Array {
  const builder = new flatbuffers.Builder(16);
  builder.finish(TemperatureBox.createTemperatureBox(builder, value));
  return builder.asUint8Array();
}

/**
 * Verifies a temperature payload through the wasm codec. Returns `undefined`
 * when the codec accepts it, and otherwise the cause.
 */
export function verifyPayload(
  codec: Codec,
  payload: Uint8Array,
): string | undefined {
  const result = codec.verify(TEMPERATURE_TYPE_ID, payload);
  if (result.ok) return undefined;
  switch (result.kind) {
    case "rule":
      return `the codec rejected the payload with rule code ${result.code}` +
        (result.code === RANGE_RULE ? " (range)" : "");
    case "malformed":
      return "the codec found the payload malformed (code 1)";
    case "unknown-type":
      return `the codec does not know type id ${TEMPERATURE_TYPE_ID}`;
  }
}

/**
 * Checks that the codec accepts 20 and rejects -41 and 86 with `RANGE_RULE`.
 * Returns `undefined` when it does, and otherwise a cause that names the
 * value and the result. The demos run it before they verify any received
 * payload, so that a codec that accepts every payload cannot pass them.
 */
export function selfTest(codec: Codec): string | undefined {
  const accepted = codec.verify(TEMPERATURE_TYPE_ID, encodeTemperature(20));
  if (!accepted.ok) {
    return `the codec rejected 20, which is inside [${LOWEST}..${HIGHEST}]: ` +
      JSON.stringify(accepted);
  }
  for (const value of [LOWEST - 1, HIGHEST + 1]) {
    const result = codec.verify(TEMPERATURE_TYPE_ID, encodeTemperature(value));
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

/** Decodes the payload of a `TemperatureBox`. */
export function decodeTemperature(payload: Uint8Array): number {
  return TemperatureBox.getRootAsTemperatureBox(
    new flatbuffers.ByteBuffer(payload),
  ).value();
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
