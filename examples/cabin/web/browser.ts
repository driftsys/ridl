// Runs the cabin temperature check in a headless browser.
//
// Usage: deno run -A browser.ts <server-executable>
//
// The program starts the server and reads its port and hash, bundles page.ts
// for the browser, serves the page and its script from 127.0.0.1, and opens
// the page in a headless Chromium-family browser. The page runs the check of
// check.ts against the server's `Cabin.temperature` signal. The program then
// stops the server and checks that it exited with status 0. It prints the
// number of samples and wraps and exits 0, or names the cause and exits 1.
// Every process, listener and temporary directory it started is stopped or
// removed before it exits.

import {
  type Browser,
  launch,
} from "../../../packages/ridl-transport-ws/e2e/browser.ts";
import { isPass, type Progress } from "./check.ts";
import { type CabinServer, startServer } from "./server.ts";

/**
 * The time from the start of the program to its end, in milliseconds. It
 * covers every step: the bundle, the start of the server, the start of the
 * browser, the page's verdict and the exit of the server. When this example
 * was written (2026-10-22), five passing runs measured on macOS with Google
 * Chrome took from 3.7 s to 8.5 s; that is a measurement, not a guarantee. The
 * page's verdict, like that of main.ts, depends on the phase of the
 * server's tick, and the worst case measured for main.ts is about 12 s.
 */
const DEADLINE_MS = 30_000;

const PAGE = `${import.meta.dirname}/page.ts`;

const HTML = `<!doctype html>
<meta charset="utf-8">
<title>cabin</title>
<script type="module" src="/page.js"></script>
`;

// What the program has started, so that the deadline can stop it too.
let dir: string | undefined;
let bundler: Deno.ChildProcess | undefined;
let killServer: (() => void) | undefined;
let server: CabinServer | undefined;
let http: Deno.HttpServer<Deno.NetAddr> | undefined;
// The browser's start, pending or done.
let launching: Promise<Browser> | undefined;

/** Bundles page.ts for the browser into `out`. */
async function bundle(out: string): Promise<void> {
  bundler = new Deno.Command(Deno.execPath(), {
    args: ["bundle", "--quiet", "--platform=browser", "--output", out, PAGE],
    // The import map and the options in deno.json are found from here.
    cwd: import.meta.dirname,
    stdout: "null",
    stderr: "piped",
  }).spawn();
  const { code, stderr } = await bundler.output();
  if (code !== 0) {
    throw new Error(`deno bundle failed: ${new TextDecoder().decode(stderr)}`);
  }
}

function kill(child: Deno.ChildProcess | undefined): void {
  try {
    child?.kill();
  } catch {
    // The process has already exited.
  }
}

/**
 * Stops every process and listener the program started and removes its
 * temporary directory. Every step runs even when an earlier one fails, so that
 * nothing outlives the program; each step that fails is reported. A browser
 * that is still starting is closed once its start ends, which takes at most the
 * 10 s that launch() waits for it.
 */
async function cleanUp(): Promise<void> {
  kill(bundler);
  killServer?.();
  const steps: [string, Promise<unknown> | undefined][] = [
    ["closing the browser", launching?.then((browser) => browser.close())],
    ["stopping the page server", http?.shutdown()],
    ["waiting for the server to exit", server?.status],
    [
      "removing the temporary directory",
      dir === undefined ? undefined : Deno.remove(dir, { recursive: true }),
    ],
  ];
  const settled = await Promise.allSettled(steps.map(([, step]) => step));
  settled.forEach((result, i) => {
    // A browser whose start failed has nothing to close. That failure is
    // reported as the cause of the run, unless an earlier cause, such as the
    // deadline, was already reported.
    if (result.status === "rejected" && !(i === 0 && !started)) {
      console.error(
        `cabin-browser: clean-up: ${steps[i][0]} failed: ${result.reason}`,
      );
    }
  });
}

// Set once launch() has resolved.
let started = false;

/** Throws unless the page's result is a progress that passed the check. */
function passed(result: unknown): Progress {
  if (typeof result !== "object" || result === null) {
    throw new Error("the page set no result");
  }
  if (!isPass(result)) {
    throw new Error(
      `the page's result is not a pass: ${JSON.stringify(result)}`,
    );
  }
  return result;
}

async function run(serverPath: string): Promise<Progress> {
  dir = await Deno.makeTempDir({ prefix: "cabin-page-" });
  const script = `${dir}/page.js`;
  await bundle(script);
  const js = await Deno.readFile(script);
  server = await startServer(serverPath, (kill) => (killServer = kill));
  http = Deno.serve(
    { hostname: "127.0.0.1", port: 0, onListen() {} },
    (request) => {
      switch (new URL(request.url).pathname) {
        case "/":
          return new Response(HTML, {
            headers: { "content-type": "text/html; charset=utf-8" },
          });
        case "/page.js":
          return new Response(js, {
            headers: { "content-type": "text/javascript; charset=utf-8" },
          });
        default:
          return new Response("not found", { status: 404 });
      }
    },
  );
  const url = `http://127.0.0.1:${http.addr.port}/` +
    `?port=${server.port}&hash=${server.hash}`;
  launching = launch();
  const browser = await launching;
  started = true;
  let result: unknown;
  try {
    result = await browser.evaluate(url, "globalThis.result");
  } catch (e) {
    // A server that exited closes the connection too. Wait briefly for its
    // exit status, so that a failure of the server is reported first.
    const exited = await Promise.race([
      server.status,
      new Promise<undefined>((resolve) =>
        setTimeout(() => resolve(undefined), 500)
      ),
    ]);
    if (exited && exited.code !== 0) {
      throw new Error(`the server exited with status ${exited.code}`);
    }
    throw e;
  }
  const progress = passed(result);
  const status = await server.stop();
  if (status.code !== 0) {
    throw new Error(`the server exited with status ${status.code}`);
  }
  return progress;
}

// The clean-up after the first cause, so that only that cause is reported
// and a later cause waits for the same clean-up.
let failing: Promise<void> | undefined;

async function failWith(message: string): Promise<never> {
  if (!failing) {
    console.error(`cabin-browser: ${message}`);
    failing = cleanUp();
  }
  await failing;
  Deno.exit(1);
}

const serverPath = Deno.args[0];
if (!serverPath) {
  console.error("cabin-browser: usage: browser.ts <server-executable>");
  Deno.exit(1);
}
const deadline = setTimeout(
  () => failWith(`the deadline of ${DEADLINE_MS} ms expired`),
  DEADLINE_MS,
);
try {
  const progress = await run(serverPath);
  clearTimeout(deadline);
  await cleanUp();
  console.log(
    `cabin-browser: received ${progress.received} sample(s), ` +
      `${progress.wraps} wrap(s)`,
  );
  console.log("cabin-browser: ok");
  Deno.exit(0);
} catch (e) {
  clearTimeout(deadline);
  await failWith((e as Error).message);
}
