import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ClientLifecycle, ClientPhase, LifecycleClient, phaseOf } from "./clientLifecycle";

// A fake language client that follows the state machine of
// vscode-languageclient 10.1.0 (lib/common/client.js): `start` moves the
// client to Starting, then to Running or StartFailed, and a second `start`
// returns the first start's promise while one is in flight. `stop` returns
// without an error in the Initial and Stopped states, returns the pending
// stop promise in Stopping, and throws "Client is not running and can't be
// stopped" in Starting and StartFailed — the only two states where it
// throws.
type State = "initial" | "starting" | "running" | "startFailed" | "stopping" | "stopped";

class FakeClient implements LifecycleClient {
  state: State = "initial";
  starts = 0;
  stops = 0;
  private onStart: Promise<void> | undefined;
  private onStop: Promise<void> | undefined;

  constructor(
    private readonly outcome: Promise<void>,
    private readonly stopOutcome: Promise<void> = Promise.resolve(),
  ) {}

  start(): Promise<void> {
    if (this.state === "stopping") {
      throw new Error("Client is currently stopping. Can only restart a full stopped client");
    }
    if (this.onStart !== undefined) return this.onStart;
    this.starts += 1;
    this.state = "starting";
    this.onStart = this.outcome.then(
      () => {
        this.state = "running";
      },
      (error: unknown) => {
        this.state = "startFailed";
        throw error;
      },
    );
    return this.onStart;
  }

  async stop(): Promise<void> {
    if (this.state === "initial" || this.state === "stopped") return;
    if (this.state === "stopping") {
      if (this.onStop === undefined) throw new Error("stopping but no stop promise available");
      return this.onStop;
    }
    if (this.state !== "running") {
      throw new Error(`Client is not running and can't be stopped. It's current state is: ${this.state}`);
    }
    this.stops += 1;
    this.state = "stopping";
    this.onStop = this.stopOutcome.then(
      () => {
        this.state = "stopped";
        this.onStart = undefined;
      },
      (error: unknown) => {
        this.state = "stopped";
        this.onStart = undefined;
        throw error;
      },
    );
    return this.onStop;
  }

  phase(): ClientPhase {
    switch (this.state) {
      case "starting":
        return "starting";
      case "running":
        return "running";
      case "startFailed":
        return "startFailed";
      default:
        return "stopped";
    }
  }

  isRunning(): boolean {
    return this.state === "running";
  }

  /**
   * The server exits and the library's error handler gives up: the client
   * will not restart itself. Ends StartFailed if the exit happened while a
   * start was in progress (client.js:1467-1468), Stopped otherwise, matching
   * the library.
   */
  serverExitedAndLibraryGaveUp(): void {
    this.state = this.state === "starting" ? "startFailed" : "stopped";
    this.onStart = undefined;
  }

  /**
   * The server exits and the library's default error handler restarts the
   * SAME client itself: state goes to Starting synchronously, and `start()`
   * from here on returns a new in-flight promise settling per `outcome`.
   */
  serverExitedAndLibraryRestarts(outcome: Promise<void>): void {
    this.state = "starting";
    this.starts += 1;
    this.onStart = outcome.then(
      () => {
        this.state = "running";
      },
      (error: unknown) => {
        this.state = "startFailed";
        throw error;
      },
    );
  }
}

/** The rejection vscode-languageclient's `stop()` gives when the server does not shut down within 2000 ms. */
function stopTimedOut(): Promise<void> {
  const promise = Promise.reject(new Error("Stopping the server timed out"));
  // The fake attaches its own handlers when `stop()` runs; this one only
  // keeps the rejection from being reported as unhandled before then.
  promise.catch(() => undefined);
  return promise;
}

/**
 * Waits until every pending promise job has run, so a queued lifecycle
 * operation has observed the client's current phase before the test changes
 * it. Without this, a test that settles the library's restart right after
 * the call cannot tell a lifecycle that waits for the restart from one that
 * does not.
 */
function queuedWorkHasRun(): Promise<void> {
  return new Promise((resolve) => setImmediate(resolve));
}

interface Deferred {
  readonly promise: Promise<void>;
  resolve(): void;
  reject(error: Error): void;
}

function deferred(): Deferred {
  let resolve!: () => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  // The fake attaches its own handlers; this one only keeps an early
  // rejection from being reported as unhandled.
  promise.catch(() => undefined);
  return { promise, resolve, reject };
}

type Outcome = "ok" | "fail" | Deferred;

/** A lifecycle whose n-th created client starts with the n-th outcome. */
function lifecycleWith(...outcomes: Outcome[]): { lifecycle: ClientLifecycle<FakeClient>; created: FakeClient[] } {
  const created: FakeClient[] = [];
  const lifecycle = new ClientLifecycle(() => {
    const outcome = outcomes[created.length];
    if (outcome === undefined) {
      throw new Error(`the test gave no outcome for client ${created.length + 1}`);
    }
    const promise =
      outcome === "ok"
        ? Promise.resolve()
        : outcome === "fail"
          ? Promise.reject(new Error("spawn ridl-lsp ENOENT"))
          : outcome.promise;
    if (outcome === "fail") promise.catch(() => undefined);
    const client = new FakeClient(promise);
    created.push(client);
    return client;
  });
  return { lifecycle, created };
}

test("a failed first start is retried when the next document opens", async () => {
  const { lifecycle, created } = lifecycleWith("fail", "ok");
  await lifecycle.startIfNeeded();
  await lifecycle.startIfNeeded();
  assert.equal(created.length, 2);
  assert.equal(lifecycle.client, created[1]);
  assert.equal(created[1].isRunning(), true);
});

test("a failed start does not reject, so activate and the dropped open-document promise see no error", async () => {
  const { lifecycle } = lifecycleWith("fail");
  await assert.doesNotReject(lifecycle.startIfNeeded());
});

test("lifecycle.client is undefined right after a failed startIfNeeded", async () => {
  const { lifecycle } = lifecycleWith("fail");
  await lifecycle.startIfNeeded();
  assert.equal(lifecycle.client, undefined);
});

test("startIfNeeded resolves true only for the call that brought the client up", async () => {
  const { lifecycle } = lifecycleWith("fail", "ok");
  const first = await lifecycle.startIfNeeded();
  const second = await lifecycle.startIfNeeded();
  const third = await lifecycle.startIfNeeded();
  assert.deepEqual([first, second, third], [false, true, false]);
});

test("concurrent document opens start one client", async () => {
  const pending = deferred();
  const { lifecycle, created } = lifecycleWith(pending);
  const calls = [lifecycle.startIfNeeded(), lifecycle.startIfNeeded()];
  pending.resolve();
  const results = await Promise.all(calls);
  assert.equal(created.length, 1);
  assert.equal(results.filter((started) => started).length, 1);
});

test("correcting ridl.serverPath after a failed first start starts a new client", async () => {
  const { lifecycle, created } = lifecycleWith("fail", "ok");
  await lifecycle.startIfNeeded();
  const restarted = await lifecycle.restart();
  assert.equal(created.length, 2);
  assert.equal(created[1].isRunning(), true);
  assert.equal(restarted, true);
});

test("restart resolves false before any start was requested, and true when it brings up the first running client after a failed first start", async () => {
  const { lifecycle } = lifecycleWith("fail", "ok");
  assert.equal(await lifecycle.restart(), false);
  await lifecycle.startIfNeeded();
  assert.equal(await lifecycle.restart(), true);
});

test("a restart while the first start is in flight waits for it, then replaces the client", async () => {
  const pending = deferred();
  const { lifecycle, created } = lifecycleWith(pending, "ok");
  const starting = lifecycle.startIfNeeded();
  const restarting = lifecycle.restart();
  pending.resolve();
  await assert.doesNotReject(restarting);
  await starting;
  assert.equal(created.length, 2);
  assert.equal(created[0].stops, 1);
  assert.equal(created[0].isRunning(), false);
  assert.equal(lifecycle.client, created[1]);
  assert.equal(created[1].isRunning(), true);
});

test("a restart while a failing first start is in flight does not throw and starts a new client", async () => {
  const pending = deferred();
  const { lifecycle, created } = lifecycleWith(pending, "ok");
  const starting = lifecycle.startIfNeeded();
  const restarting = lifecycle.restart();
  pending.reject(new Error("spawn ridl-lsp ENOENT"));
  await assert.doesNotReject(restarting);
  await starting;
  assert.equal(created.length, 2);
  assert.equal(lifecycle.client, created[1]);
  assert.equal(created[1].isRunning(), true);
});

test("a restart after a successful start stops the old client and starts a new one", async () => {
  const { lifecycle, created } = lifecycleWith("ok", "ok");
  await lifecycle.startIfNeeded();
  await lifecycle.restart();
  assert.equal(created.length, 2);
  assert.equal(created[0].stops, 1);
  assert.equal(created[1].isRunning(), true);
});

test("a restart before any typl, ridl or rsdl document opened creates no client", async () => {
  const { lifecycle, created } = lifecycleWith();
  await assert.doesNotReject(lifecycle.restart());
  assert.equal(created.length, 0);
  assert.equal(lifecycle.client, undefined);
});

test("a failed restart does not reject, and the next document open retries", async () => {
  const { lifecycle, created } = lifecycleWith("ok", "fail", "ok");
  await lifecycle.startIfNeeded();
  const restarted = await lifecycle.restart();
  assert.equal(restarted, false);
  await lifecycle.startIfNeeded();
  assert.equal(created.length, 3);
  assert.equal(created[2].isRunning(), true);
});

test("lifecycle.client is undefined right after a failed restart, and restart resolved false", async () => {
  const { lifecycle } = lifecycleWith("ok", "fail");
  await lifecycle.startIfNeeded();
  const restarted = await lifecycle.restart();
  assert.equal(restarted, false);
  assert.equal(lifecycle.client, undefined);
});

test("stop after a failed start does not throw", async () => {
  const { lifecycle } = lifecycleWith("fail");
  await lifecycle.startIfNeeded();
  await assert.doesNotReject(lifecycle.stop());
  assert.equal(lifecycle.client, undefined);
});

test("stop while the start is in flight waits for it, then stops the client", async () => {
  const pending = deferred();
  const { lifecycle, created } = lifecycleWith(pending);
  const starting = lifecycle.startIfNeeded();
  const stopping = lifecycle.stop();
  pending.resolve();
  await assert.doesNotReject(stopping);
  await starting;
  assert.equal(created[0].stops, 1);
  assert.equal(created[0].isRunning(), false);
});

test("after a successful start then stop, lifecycle.client is undefined and the client was stopped once", async () => {
  const { lifecycle, created } = lifecycleWith("ok");
  await lifecycle.startIfNeeded();
  await lifecycle.stop();
  assert.equal(lifecycle.client, undefined);
  assert.equal(created[0].stops, 1);
});

test("after the library gives up restarting a crashed server, the next document open creates a second client", async () => {
  const { lifecycle, created } = lifecycleWith("ok", "ok");
  await lifecycle.startIfNeeded();
  created[0].serverExitedAndLibraryGaveUp();
  const started = await lifecycle.startIfNeeded();
  assert.equal(started, true);
  assert.equal(created.length, 2);
  assert.equal(lifecycle.client, created[1]);
});

test("while the library restarts the server itself, startIfNeeded creates no second client and waits for it", async () => {
  const { lifecycle, created } = lifecycleWith("ok", "ok");
  await lifecycle.startIfNeeded();
  const restarting = deferred();
  created[0].serverExitedAndLibraryRestarts(restarting.promise);
  let settled = false;
  const started = lifecycle.startIfNeeded().then((result) => {
    settled = true;
    return result;
  });
  await queuedWorkHasRun();
  assert.equal(settled, false, "startIfNeeded resolved before the library's own restart settled");
  restarting.resolve();
  assert.equal(await started, false);
  assert.equal(created.length, 1);
  assert.equal(created[0].isRunning(), true);
});

test("while the library restarts the server itself but that restart fails, startIfNeeded creates a new client", async () => {
  const { lifecycle, created } = lifecycleWith("ok", "ok");
  await lifecycle.startIfNeeded();
  const restarting = deferred();
  created[0].serverExitedAndLibraryRestarts(restarting.promise);
  const started = lifecycle.startIfNeeded();
  await queuedWorkHasRun();
  restarting.reject(new Error("spawn ridl-lsp ENOENT"));
  assert.equal(await started, true);
  assert.equal(created.length, 2);
  assert.equal(created[0].isRunning(), false);
  assert.equal(created[1].isRunning(), true);
});

test("while the library restarts the server itself, restart waits for it, stops that client once, and starts a new one", async () => {
  const { lifecycle, created } = lifecycleWith("ok", "ok");
  await lifecycle.startIfNeeded();
  const restarting = deferred();
  created[0].serverExitedAndLibraryRestarts(restarting.promise);
  const doingRestart = lifecycle.restart();
  await queuedWorkHasRun();
  restarting.resolve();
  assert.equal(await doingRestart, true);
  assert.equal(created.length, 2);
  assert.equal(created[0].stops, 1);
  assert.equal(created[1].isRunning(), true);
});

test("while the library restarts the server itself but that restart fails, restart calls stop on nothing and starts a new client", async () => {
  const { lifecycle, created } = lifecycleWith("ok", "ok");
  await lifecycle.startIfNeeded();
  const restarting = deferred();
  created[0].serverExitedAndLibraryRestarts(restarting.promise);
  const doingRestart = lifecycle.restart();
  await queuedWorkHasRun();
  restarting.reject(new Error("spawn ridl-lsp ENOENT"));
  assert.equal(await doingRestart, true);
  assert.equal(created.length, 2);
  assert.equal(created[0].stops, 0);
  assert.equal(created[1].isRunning(), true);
});

test("while the library restarts the server itself, stop waits for it, stops that client once, and leaves client undefined", async () => {
  const { lifecycle, created } = lifecycleWith("ok");
  await lifecycle.startIfNeeded();
  const restarting = deferred();
  created[0].serverExitedAndLibraryRestarts(restarting.promise);
  const stopping = lifecycle.stop();
  await queuedWorkHasRun();
  restarting.resolve();
  await stopping;
  assert.equal(created[0].stops, 1);
  assert.equal(lifecycle.client, undefined);
});

test("while the library restarts the server itself but that restart fails, stop does not throw and leaves client undefined", async () => {
  const { lifecycle, created } = lifecycleWith("ok");
  await lifecycle.startIfNeeded();
  const restarting = deferred();
  created[0].serverExitedAndLibraryRestarts(restarting.promise);
  const stopping = lifecycle.stop();
  await queuedWorkHasRun();
  restarting.reject(new Error("spawn ridl-lsp ENOENT"));
  await assert.doesNotReject(stopping);
  assert.equal(created[0].stops, 0);
  assert.equal(lifecycle.client, undefined);
});

test("a restart whose stop() rejects still starts the new client and resolves true; the old client ends stopped", async () => {
  const created: FakeClient[] = [];
  const lifecycle = new ClientLifecycle(() => {
    const client = new FakeClient(Promise.resolve(), stopTimedOut());
    created.push(client);
    return client;
  });
  await lifecycle.startIfNeeded();
  const restarted = await lifecycle.restart();
  assert.equal(restarted, true);
  assert.equal(created.length, 2);
  assert.equal(created[0].stops, 1);
  assert.equal(created[0].isRunning(), false);
  assert.equal(created[1].isRunning(), true);
});

test("a stop whose client's stop() rejects does not reject and leaves client undefined", async () => {
  const created: FakeClient[] = [];
  const lifecycle = new ClientLifecycle(() => {
    const client = new FakeClient(Promise.resolve(), stopTimedOut());
    created.push(client);
    return client;
  });
  await lifecycle.startIfNeeded();
  await assert.doesNotReject(lifecycle.stop());
  assert.equal(lifecycle.client, undefined);
});

test("the queue keeps running after an operation rejects", async () => {
  let calls = 0;
  const lifecycle = new ClientLifecycle<FakeClient>(() => {
    calls += 1;
    if (calls === 1) throw new Error("constructor error");
    const client = new FakeClient(Promise.resolve());
    return client;
  });
  await assert.rejects(lifecycle.startIfNeeded());
  const started = await lifecycle.startIfNeeded();
  assert.equal(started, true);
  assert.ok(lifecycle.client);
});

test("a constructor error during restart rejects restart, and the queue keeps running afterward", async () => {
  let calls = 0;
  const lifecycle = new ClientLifecycle<FakeClient>(() => {
    calls += 1;
    if (calls === 2) throw new Error("constructor error");
    return new FakeClient(Promise.resolve());
  });
  await lifecycle.startIfNeeded();
  await assert.rejects(lifecycle.restart(), /constructor error/);
  const started = await lifecycle.startIfNeeded();
  assert.equal(started, true);
  assert.ok(lifecycle.client);
});

test("phaseOf maps every vscode-languageclient State number to its ClientPhase", () => {
  // The library's own numbers (lib/common/client.d.ts): Stopped = 1, Running = 2, Starting = 3, StartFailed = 4.
  assert.equal(phaseOf(3), "starting");
  assert.equal(phaseOf(2), "running");
  assert.equal(phaseOf(4), "startFailed");
  assert.equal(phaseOf(1), "stopped");
});

test("phaseOf throws for a state number the library does not declare", () => {
  assert.throws(() => phaseOf(99), /unhandled vscode-languageclient state/);
});

test("FakeClient.stop called again while stopping does not stop it a second time", async () => {
  const client = new FakeClient(Promise.resolve());
  await client.start();
  const first = client.stop();
  const second = client.stop();
  await Promise.all([first, second]);
  assert.equal(client.stops, 1);
  assert.equal(client.phase(), "stopped");
});

test("FakeClient.start throws while the client is stopping, matching vscode-languageclient", async () => {
  const client = new FakeClient(Promise.resolve());
  await client.start();
  const stopping = client.stop();
  assert.throws(() => client.start(), /currently stopping/);
  await stopping;
});

test("FakeClient.start after a stop starts a new promise instead of returning the old one, matching vscode-languageclient", async () => {
  const client = new FakeClient(Promise.resolve());
  await client.start();
  await client.stop();
  await client.start();
  assert.equal(client.starts, 2);
  assert.equal(client.isRunning(), true);
});

test("FakeClient.serverExitedAndLibraryGaveUp during the first start ends startFailed, matching vscode-languageclient", () => {
  const client = new FakeClient(new Promise(() => undefined));
  void client.start();
  client.serverExitedAndLibraryGaveUp();
  assert.equal(client.phase(), "startFailed");
});

test("FakeClient.serverExitedAndLibraryGaveUp while running ends stopped", async () => {
  const client = new FakeClient(Promise.resolve());
  await client.start();
  client.serverExitedAndLibraryGaveUp();
  assert.equal(client.phase(), "stopped");
});
