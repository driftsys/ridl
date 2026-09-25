import { strict as assert } from "node:assert";
import { test } from "node:test";
import { ClientLifecycle, LifecycleClient } from "./clientLifecycle";

// A fake language client that follows the state machine of
// vscode-languageclient 10.1.0 (lib/common/client.js): `start` moves the
// client to Starting, then to Running or StartFailed, and a second `start`
// returns the first start's promise. `stop` returns without an error in the
// Initial and Stopped states and throws "Client is not running and can't be
// stopped" in every state other than Running.
type State = "initial" | "starting" | "running" | "startFailed" | "stopped";

class FakeClient implements LifecycleClient {
  state: State = "initial";
  starts = 0;
  stops = 0;
  private onStart: Promise<void> | undefined;

  constructor(private readonly outcome: Promise<void>) {}

  start(): Promise<void> {
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
    if (this.state !== "running") {
      throw new Error(`Client is not running and can't be stopped. It's current state is: ${this.state}`);
    }
    this.stops += 1;
    this.state = "stopped";
  }

  isRunning(): boolean {
    return this.state === "running";
  }
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
  await lifecycle.startIfNeeded().catch(() => false);
  await lifecycle.startIfNeeded();
  assert.equal(created.length, 2);
  assert.equal(lifecycle.client, created[1]);
  assert.equal(created[1].isRunning(), true);
});

test("a failed start does not reject, so activate and the dropped open-document promise see no error", async () => {
  const { lifecycle } = lifecycleWith("fail");
  await assert.doesNotReject(lifecycle.startIfNeeded());
});

test("startIfNeeded resolves true only for the call that brought the client up", async () => {
  const { lifecycle } = lifecycleWith("fail", "ok");
  const first = await lifecycle.startIfNeeded().catch(() => "rejected");
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
  await lifecycle.startIfNeeded().catch(() => false);
  await assert.doesNotReject(lifecycle.restart());
  assert.equal(created.length, 2);
  assert.equal(created[1].isRunning(), true);
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
  const starting = lifecycle.startIfNeeded().catch(() => false);
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
  await assert.doesNotReject(lifecycle.restart());
  await lifecycle.startIfNeeded();
  assert.equal(created.length, 3);
  assert.equal(created[2].isRunning(), true);
});

test("stop after a failed start does not throw", async () => {
  const { lifecycle } = lifecycleWith("fail");
  await lifecycle.startIfNeeded().catch(() => false);
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
