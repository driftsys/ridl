// A wait for one value from a listener, with a time limit that starts when
// the test starts it rather than when the wait is created.

/** A wait created by {@link waiter}. */
export interface Waiter<T> {
  /** Resolves with the first value given to `accept`. */
  promise: Promise<T>;
  /** Resolves the wait. Later values are ignored. */
  accept(value: T): void;
  /**
   * Starts the time limit: the wait rejects when `ms` milliseconds pass
   * before a value arrives. Does nothing once the wait has a value.
   */
  start(): void;
  /** Stops the time limit, if it is running. */
  cancel(): void;
}

/**
 * Creates a wait that can be registered with a listener before the action
 * that produces its value, so that a value that arrives early is not missed,
 * while the time limit of `ms` milliseconds counts from {@link Waiter.start}.
 */
export function waiter<T>(what: string, ms: number): Waiter<T> {
  let resolve!: (value: T) => void;
  let reject!: (e: Error) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  // A rejection that no one awaits, because an earlier step already failed,
  // is not reported as unhandled.
  promise.catch(() => {});
  let settled = false;
  let timer: ReturnType<typeof setTimeout> | undefined;
  return {
    promise,
    accept(value) {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      resolve(value);
    },
    start() {
      if (settled || timer !== undefined) return;
      timer = setTimeout(() => {
        settled = true;
        reject(new Error(`no ${what} within ${ms} ms`));
      }, ms);
    },
    cancel() {
      clearTimeout(timer);
    },
  };
}
