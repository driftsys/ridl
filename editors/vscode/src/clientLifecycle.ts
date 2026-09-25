// When the extension starts, restarts and stops the language client. Pure —
// no `vscode` import — so it is unit-tested directly against a fake client.
//
// startIfNeeded, restart and stop all run through one internal promise
// chain (`queue`), so an operation issued while another is in flight waits
// for it to settle before running. The chain never breaks: each queued
// operation is run whether the previous one resolved or rejected, and
// startIfNeeded itself never rejects (see its own comment).

/** The part of a vscode-languageclient `LanguageClient` the lifecycle drives. */
export interface LifecycleClient {
  start(): Promise<void>;
  stop(): Promise<void>;
  isRunning(): boolean;
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
   * Starts the client the first time a typl, ridl or rsdl document opens; a
   * no-op afterward while the client is running. Resolves true when this
   * call brought the client up. When the start fails, drops the client and
   * resolves false rather than rejecting: vscode-languageclient already
   * shows the start error to the user itself (`this.error(..., 'force')` in
   * its `start`), so this method does not surface it again. Dropping the
   * client lets the next document open retry with a fresh one, since a
   * client whose start failed cannot be restarted — the library's `start()`
   * just returns the same rejected promise again.
   */
  startIfNeeded(): Promise<boolean> {
    return this.enqueue(async () => {
      this.started = true;
      if (this.current?.isRunning()) return false;
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
   * Replaces the client with a new one, so a changed `ridl.serverPath`
   * takes effect. A no-op until a start has been requested. Stops the
   * current client only when it is running — vscode-languageclient throws
   * when `stop()` is called on a client that is not running (every state
   * other than Running, Initial and Stopped). A failed start drops the
   * client and does not reject, so the next document open retries.
   */
  restart(): Promise<void> {
    return this.enqueue(async () => {
      if (!this.started) return;
      if (this.current?.isRunning()) {
        await this.current.stop();
      }
      this.current = this.create();
      try {
        await this.current.start();
      } catch {
        this.current = undefined;
      }
    });
  }

  /**
   * Stops the client, for extension deactivation. Stops it only when it is
   * running, for the same reason as `restart`. Resolves without error after
   * a failed start.
   */
  stop(): Promise<void> {
    return this.enqueue(async () => {
      if (this.current?.isRunning()) {
        await this.current.stop();
      }
      this.current = undefined;
    });
  }

  /** Queues `operation` after every previously queued one has settled, success or failure. */
  private enqueue<T>(operation: () => Promise<T>): Promise<T> {
    const result = this.queue.then(operation, operation);
    this.queue = result.then(
      () => undefined,
      () => undefined,
    );
    return result;
  }
}
