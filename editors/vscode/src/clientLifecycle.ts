// When the extension starts, restarts and stops the language client. Pure —
// no `vscode` import — so it is unit-tested directly against a fake client.
//
// vscode-languageclient (10.1.0) throws from `stop()` only when the client is
// in the Starting or StartFailed state; it returns without error when the
// client is Initial or Stopped, and returns the pending stop promise when the
// client is already Stopping. When the server process exits, the library's
// default error handler can restart the SAME client by itself: the state
// goes back to Initial, then `start()` sets it to Starting again inside the
// same synchronous call. This lifecycle recognizes that case (`phase()` is
// "starting" on a client this lifecycle did not just start itself) and waits
// for the library's own restart instead of creating a second client.
//
// A client whose start failed is dropped rather than kept for reuse: after a
// spawn failure, the library's `start()` keeps returning the same rejected
// promise, so that client can never come up again. The next document open
// (or a `restart()`) constructs a fresh one.
//
// startIfNeeded, restart and stop all run through one internal promise chain
// (`queue`), so an operation issued while another is in flight waits for it
// to settle before running. `queue` is normalized to a promise that never
// rejects after each operation, so the chain never breaks and the next
// enqueued operation always runs. startIfNeeded and restart never reject when
// a start fails (see their own comments) — only a constructor error from
// `create()` propagates.

/** The phase vscode-languageclient's public `State` enum reports, spelled without the `State.` import this module avoids. */
export type ClientPhase = "starting" | "running" | "startFailed" | "stopped";

/**
 * Maps a vscode-languageclient client's numeric `State` to the `ClientPhase`
 * this lifecycle reads. Takes a plain number rather than the library's own
 * `State` enum, because importing anything from `vscode-languageclient/node`
 * pulls in a transitive `import "vscode"` that only resolves inside a running
 * extension host, and this module stays free of that so it can be
 * unit-tested directly. The numbers are the library's own
 * (lib/common/client.d.ts): Stopped = 1, Running = 2, Starting = 3,
 * StartFailed = 4. A numeric `State` enum member is assignable to `number`,
 * so a caller passes `this.state` straight through.
 */
export function phaseOf(state: number): ClientPhase {
  switch (state) {
    case 3:
      return "starting";
    case 2:
      return "running";
    case 4:
      return "startFailed";
    case 1:
      return "stopped";
    default:
      throw new Error(`unhandled vscode-languageclient state: ${state}`);
  }
}

/** The part of a vscode-languageclient `LanguageClient` the lifecycle drives. */
export interface LifecycleClient {
  start(): Promise<void>;
  stop(): Promise<void>;
  phase(): ClientPhase;
}

/** Owns the one language client and decides when to create, start and stop it. */
export class ClientLifecycle<C extends LifecycleClient> {
  private queue: Promise<unknown> = Promise.resolve();
  private started = false;
  private current: C | undefined;

  constructor(private readonly create: () => C) {}

  /** The current client, if one has been created and its start has not failed. */
  get client(): C | undefined {
    return this.current;
  }

  /**
   * Waits out a restart vscode-languageclient is already running by itself.
   * After the server process exits, the library's default error handler can
   * restart the SAME client: its state goes back to Initial, then `start()`
   * sets it to Starting again before this lifecycle observes anything.
   * Because the lifecycle's own start is always awaited inside its own
   * operation, a "starting" phase found here means the library did this
   * restart on its own. `current.start()` then returns the library's
   * in-flight promise rather than starting a second one; a rejection is
   * ignored, since the library has already logged it.
   */
  private async settle(): Promise<void> {
    if (this.current?.phase() === "starting") {
      await this.current.start().catch(() => undefined);
    }
  }

  /**
   * Starts the client the first time a typl, ridl or rsdl document opens; a
   * no-op afterward while the client is running. Resolves true when this
   * call brought the client up. First waits for a restart the library is
   * already doing on its own (see `settle`). When the start fails, drops the
   * client and resolves false rather than rejecting: vscode-languageclient
   * already shows the start error to the user itself (`this.error(...,
   * 'force')` in its `start`), so this method does not surface it again. A
   * client left in "stopped" (the library gave up restarting it) or
   * "startFailed" (the library's `start()` would just return the same
   * rejected promise again) is likewise replaced by a new one, since
   * `current` is reused only when it is "running". `create()` runs outside
   * the try: a constructor error propagates to the caller instead of being
   * swallowed here.
   */
  startIfNeeded(): Promise<boolean> {
    return this.enqueue(async () => {
      this.started = true;
      await this.settle();
      if (this.current?.phase() === "running") return false;
      this.current = this.create();
      try {
        await this.current.start();
        return true;
      } catch {
        this.current = undefined;
        return false;
      }
    });
  }

  /**
   * Replaces the client with a new one, so a changed `ridl.serverPath` takes
   * effect. A no-op that resolves false until a start has been requested.
   * Resolves true when the new client comes up. First waits for a restart
   * the library is already doing on its own (see `settle`), then stops the
   * current client only when it is running — vscode-languageclient's
   * `stop()` throws in the Starting and StartFailed states — ignoring a
   * rejection (for example a stop that timed out), since the library has
   * already logged it.
   * A failed start on the new client drops it and resolves false rather than
   * rejecting, so the next document open retries.
   */
  restart(): Promise<boolean> {
    return this.enqueue(async () => {
      if (!this.started) return false;
      await this.settle();
      if (this.current?.phase() === "running") {
        await this.current.stop().catch(() => undefined);
      }
      this.current = this.create();
      try {
        await this.current.start();
        return true;
      } catch {
        this.current = undefined;
        return false;
      }
    });
  }

  /**
   * Stops the client, for extension deactivation. First waits for a restart
   * the library is already doing on its own (see `settle`), then stops the
   * client only when it is running, for the same reason as `restart`,
   * ignoring a rejection since the library has already logged it. Resolves
   * without error after a failed start.
   */
  stop(): Promise<void> {
    return this.enqueue(async () => {
      await this.settle();
      if (this.current?.phase() === "running") {
        await this.current.stop().catch(() => undefined);
      }
      this.current = undefined;
    });
  }

  /** Queues `operation` after every previously queued one has settled, success or failure. */
  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.queue.then(operation);
    this.queue = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }
}
