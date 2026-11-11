import type { Detection, SignalCheck } from "./checks.ts";

/** The sender's envelope: a signed 64-bit stamp and an unsigned 64-bit sequence. */
export interface Envelope {
  /** The timestamp, a signed 64-bit integer. */
  stamp: bigint;
  /** The sequence number, an unsigned 64-bit integer. */
  seq: bigint;
}

/** Optional limits on waiting for the answer of a port operation. */
export interface WaitOptions {
  /** Milliseconds to wait: finite, greater than 0 and at most 2147483647. */
  bound?: number;
  /** Aborting rejects with this signal's reason. */
  signal?: AbortSignal;
}

/** The limits of one call. */
export interface CallOptions {
  /** Milliseconds: finite, greater than 0 and at most 2147483647. */
  readonly bound: number;
  /** Aborting rejects the call with the signal's reason. */
  readonly signal?: AbortSignal;
}

/**
 * The state of a signal, as one publish carried it: bytes, not a decoded value.
 */
export interface RawSample {
  /** `init` before the first publication, `live` for a value, `invalid` for an invalidation. */
  provenance: "init" | "live" | "invalid";
  /** The envelope of the publish. */
  envelope: Envelope;
  /** The value under `live`, and `null` under `init` and `invalid`. */
  payload: Uint8Array | null;
  /** The last passing payload, or `null` when the init value applies. */
  lastGood: Uint8Array | null;
  /**
   * Set when the port itself invalidated the sample: the payload was longer
   * than the member's limit or refused by its check. The provenance is then
   * `invalid` and `payload` is `null`.
   */
  detected?: Detection;
}

/** One occurrence of a subscribed event, as bytes. */
export interface RawOccurrence {
  /** The interface number. */
  iface: number;
  /** The member ordinal. */
  ord: number;
  /** The envelope of the occurrence. */
  envelope: Envelope;
  /**
   * The value, as the provider encoded it; empty when the port refused it
   * (`detected` is set).
   */
  payload: Uint8Array;
  /** Set when the port refused the value because it was longer than the member's limit. */
  detected?: "corrupt";
}

/** The state of a signal with its value decoded. */
export interface Sample<T> {
  /** `init` before the first publication, `live` for a value, `invalid` for an invalidation. */
  readonly provenance: "init" | "live" | "invalid";
  /** The envelope of the publish. */
  readonly envelope: Envelope;
  /**
   * The decoded live value. Under `init` and `invalid` it is the last good
   * value when there is one, and the init value of the type otherwise, so it
   * is never absent.
   */
  readonly value: T;
  /** Set when the port itself invalidated the sample; see {@link RawSample.detected}. */
  readonly detected?: Detection;
}

/** One occurrence of an event with its value decoded. */
export interface Occurrence<T> {
  /** The interface number. */
  readonly iface: number;
  /** The member ordinal. */
  readonly ord: number;
  /** The envelope of the occurrence. */
  readonly envelope: Envelope;
  /** The decoded value; absent when the port refused the occurrence (`detected` is set). */
  readonly value?: T;
  /** Set when the port refused the value; see {@link RawOccurrence.detected}. */
  readonly detected?: "corrupt";
}

/** A port that is attached to one catalog. */
export interface Attached {
  /** The 32-byte hash of the catalog the port is attached to. */
  readonly catalogHash: Uint8Array;
}

/** Sends commands and queries as bytes and returns the reply bytes. */
export interface Caller extends Attached {
  /**
   * Sends a command and resolves when the provider accepted it. Aborting after
   * the send rejects the promise, but a provider that received the command may
   * still run it.
   */
  command(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): Promise<void>;
  /** Sends a query and resolves with the reply bytes. */
  query(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): Promise<Uint8Array>;
}

/** Reads and watches signals as bytes. */
export interface SignalReader extends Attached {
  /** Registers the check that runs on every live payload of the signal. */
  checkWith(iface: number, ord: number, check: SignalCheck): void;
  /** Subscribes to the signal and resolves when the provider answered. */
  subscribe(iface: number, ord: number, opts?: WaitOptions): Promise<void>;
  /** Ends the subscription to the signal. */
  unsubscribe(iface: number, ord: number): void;
  /** The held sample, or undefined before the first publish. */
  sample(iface: number, ord: number): RawSample | undefined;
  /** Waits for the next publish of the signal. */
  read(iface: number, ord: number, opts?: WaitOptions): Promise<RawSample>;
  /** Called for every accepted publish of the signal; returns the removal. */
  watch(
    iface: number,
    ord: number,
    listener: (s: RawSample) => void,
  ): () => void;
}

/** Receives the occurrences of events as bytes. */
export interface EventSource extends Attached {
  /** Subscribes to the event and resolves when the provider answered. */
  subscribe(iface: number, ord: number, opts?: WaitOptions): Promise<void>;
  /** Ends the subscription to the event. */
  unsubscribe(iface: number, ord: number): void;
  /** Called for every accepted occurrence of the interface, in order; returns the removal. */
  listen(iface: number, listener: (o: RawOccurrence) => void): () => void;
}

/** A {@link Caller} that also opens byte streams. */
export interface StreamCaller extends Caller {
  /** Opens a reply stream; cancellation reaches its upstream transport. */
  queryStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): ReadableStream<Uint8Array>;
  /**
   * Opens concurrent upload and reply streams. Upload close leaves the reply
   * open; aborting upload or canceling reply interrupts the call.
   */
  queryDuplex(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): { upload: WritableStream<Uint8Array>; stream: ReadableStream<Uint8Array> };
  /** Opens an upload whose acceptance does not confirm provider consumption. */
  commandStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): { upload: WritableStream<Uint8Array>; accepted: Promise<void> };
  /** Opens an upload and returns the eventual plain reply bytes. */
  queryWithUpload(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> };
}
