import type {
  Caller,
  CallOptions,
  EventSource,
  RawOccurrence,
  RawSample,
  SignalCheck,
  SignalReader,
  StreamCaller,
  WaitOptions,
} from "../mod.ts";

/** One call that the fake received. */
export interface Recorded {
  readonly method: string;
  readonly iface: number;
  readonly ord?: number;
  readonly args?: Uint8Array;
  readonly opts?: CallOptions | WaitOptions;
}

const key = (iface: number, ord: number) => `${iface}/${ord}`;

/** Rejects with the signal's reason when it aborts before `promise` settles. */
function abortable<T>(promise: Promise<T>, signal?: AbortSignal): Promise<T> {
  if (!signal) return promise;
  if (signal.aborted) return Promise.reject(signal.reason);
  return new Promise<T>((resolve, reject) => {
    const onAbort = () => reject(signal.reason);
    signal.addEventListener("abort", onAbort, { once: true });
    promise.then(resolve, reject).finally(() =>
      signal.removeEventListener("abort", onAbort)
    );
  });
}

/**
 * An in-process implementation of every port. A test scripts the handlers,
 * reads `calls` and publishes samples and occurrences.
 */
export class FakePort
  implements Caller, SignalReader, EventSource, StreamCaller {
  readonly catalogHash: Uint8Array;
  /** Every call received, in order. */
  readonly calls: Recorded[] = [];
  /** The checks registered with `checkWith`, by `iface/ord`. */
  readonly checks = new Map<string, SignalCheck>();
  /** Scripts the result of `command`; the default resolves. */
  onCommand: (call: Recorded) => Promise<void> = () => Promise.resolve();
  /** Scripts the result of `query`; the default resolves with no bytes. */
  onQuery: (call: Recorded) => Promise<Uint8Array> = () =>
    Promise.resolve(new Uint8Array());
  /** Scripts the result of `subscribe`; the default resolves. */
  onSubscribe: (call: Recorded) => Promise<void> = () => Promise.resolve();

  #held = new Map<string, RawSample>();
  #waiting = new Map<string, Array<(s: RawSample) => void>>();
  #watchers = new Map<string, Set<(s: RawSample) => void>>();
  #listeners = new Map<number, Set<(o: RawOccurrence) => void>>();

  constructor(catalogHash: Uint8Array = new Uint8Array(32)) {
    this.catalogHash = catalogHash;
  }

  command(iface: number, ord: number, args: Uint8Array, opts: CallOptions) {
    const call = { method: "command", iface, ord, args, opts };
    this.calls.push(call);
    return abortable(this.onCommand(call), opts.signal);
  }

  query(iface: number, ord: number, args: Uint8Array, opts: CallOptions) {
    const call = { method: "query", iface, ord, args, opts };
    this.calls.push(call);
    return abortable(this.onQuery(call), opts.signal);
  }

  checkWith(iface: number, ord: number, check: SignalCheck): void {
    this.calls.push({ method: "checkWith", iface, ord });
    this.checks.set(key(iface, ord), check);
  }

  subscribe(iface: number, ord: number, opts?: WaitOptions): Promise<void> {
    const call = { method: "subscribe", iface, ord, opts };
    this.calls.push(call);
    return abortable(this.onSubscribe(call), opts?.signal);
  }

  unsubscribe(iface: number, ord: number): void {
    this.calls.push({ method: "unsubscribe", iface, ord });
  }

  sample(iface: number, ord: number): RawSample | undefined {
    return this.#held.get(key(iface, ord));
  }

  read(iface: number, ord: number, opts?: WaitOptions): Promise<RawSample> {
    this.calls.push({ method: "read", iface, ord, opts });
    const k = key(iface, ord);
    const next = new Promise<RawSample>((resolve) => {
      const list = this.#waiting.get(k) ?? [];
      list.push(resolve);
      this.#waiting.set(k, list);
    });
    return abortable(next, opts?.signal);
  }

  watch(
    iface: number,
    ord: number,
    listener: (s: RawSample) => void,
  ): () => void {
    const k = key(iface, ord);
    const set = this.#watchers.get(k) ?? new Set();
    set.add(listener);
    this.#watchers.set(k, set);
    return () => void set.delete(listener);
  }

  listen(iface: number, listener: (o: RawOccurrence) => void): () => void {
    const set = this.#listeners.get(iface) ?? new Set();
    set.add(listener);
    this.#listeners.set(iface, set);
    return () => void set.delete(listener);
  }

  /** Holds the sample, resolves the pending reads and calls the watchers. */
  publish(iface: number, ord: number, sample: RawSample): void {
    const k = key(iface, ord);
    this.#held.set(k, sample);
    const waiting = this.#waiting.get(k) ?? [];
    this.#waiting.delete(k);
    for (const resolve of waiting) resolve(sample);
    for (const listener of [...(this.#watchers.get(k) ?? [])]) {
      listener(sample);
    }
  }

  /** Calls the listeners of the occurrence's interface. */
  occur(occurrence: RawOccurrence): void {
    for (const listener of [...(this.#listeners.get(occurrence.iface) ?? [])]) {
      listener(occurrence);
    }
  }

  /** The number of watchers of a signal still registered. */
  watcherCount(iface: number, ord: number): number {
    return this.#watchers.get(key(iface, ord))?.size ?? 0;
  }

  queryStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): ReadableStream<Uint8Array> {
    this.calls.push({ method: "queryStream", iface, ord, args, opts });
    return ReadableStream.from([]);
  }

  queryDuplex(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): {
    upload: WritableStream<Uint8Array>;
    stream: ReadableStream<Uint8Array>;
  } {
    this.calls.push({ method: "queryDuplex", iface, ord, args, opts });
    return { upload: new WritableStream(), stream: ReadableStream.from([]) };
  }

  commandStream(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): { upload: WritableStream<Uint8Array>; accepted: Promise<void> } {
    this.calls.push({ method: "commandStream", iface, ord, args, opts });
    return { upload: new WritableStream(), accepted: Promise.resolve() };
  }

  queryWithUpload(
    iface: number,
    ord: number,
    args: Uint8Array,
    opts: CallOptions,
  ): { upload: WritableStream<Uint8Array>; reply: Promise<Uint8Array> } {
    this.calls.push({ method: "queryWithUpload", iface, ord, args, opts });
    return {
      upload: new WritableStream(),
      reply: Promise.resolve(new Uint8Array()),
    };
  }
}
