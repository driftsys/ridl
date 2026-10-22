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
import type { Progress } from "./check.ts";
import { type CabinServer, startServer } from "./server.ts";

/**
 * The time the page has to give its verdict, in milliseconds. It is the
 * deadline of main.ts, for the same reason: the worst pass measured is about
 * 12 s.
 */
const LIMIT_MS = 30_000;

const PAGE = `${import.meta.dirname}/page.ts`;

const HTML = `<!doctype html>
<meta charset="utf-8">
<title>cabin</title>
<script type="module" src="/page.js"></script>
`;

/** Bundles page.ts for the browser into `out`. */
async function bundle(out: string): Promise<void> {
  const { code, stderr } = await new Deno.Command(Deno.execPath(), {
    args: ["bundle", "--quiet", "--platform=browser", "--output", out, PAGE],
    // The import map and the options in deno.json are found from here.
    cwd: import.meta.dirname,
    stdout: "null",
    stderr: "piped",
  }).output();
  if (code !== 0) {
    throw new Error(`deno bundle failed: ${new TextDecoder().decode(stderr)}`);
  }
}

/** Rejects after `ms` unless `promise` settles first. */
async function limit<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () => reject(new Error(`the page gave no verdict within ${ms} ms`)),
      ms,
    );
  });
  try {
    return await Promise.race([promise, late]);
  } finally {
    clearTimeout(timer);
  }
}

async function run(serverPath: string): Promise<Progress> {
  const dir = await Deno.makeTempDir({ prefix: "cabin-page-" });
  let server: CabinServer | undefined;
  let http: Deno.HttpServer<Deno.NetAddr> | undefined;
  let browser: Browser | undefined;
  try {
    const script = `${dir}/page.js`;
    await bundle(script);
    const js = await Deno.readFile(script);
    server = await startServer(serverPath);
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
    browser = await launch();
    const progress = await limit(
      browser.evaluate(url, "globalThis.result"),
      LIMIT_MS,
    ) as Progress;
    const status = await server.stop();
    server = undefined;
    if (status.code !== 0) {
      throw new Error(`the server exited with status ${status.code}`);
    }
    return progress;
  } finally {
    // Every step runs even when an earlier one fails, so that no process,
    // listener or directory outlives the program.
    server?.kill();
    await Promise.allSettled([
      browser?.close(),
      http?.shutdown(),
      server?.status,
      Deno.remove(dir, { recursive: true }),
    ]);
  }
}

const serverPath = Deno.args[0];
if (!serverPath) {
  console.error("cabin-browser: usage: browser.ts <server-executable>");
  Deno.exit(1);
}
try {
  const progress = await run(serverPath);
  console.log(
    `cabin-browser: received ${progress.received} sample(s), ` +
      `${progress.wraps} wrap(s)`,
  );
  console.log("cabin-browser: ok");
  Deno.exit(0);
} catch (e) {
  console.error(`cabin-browser: ${(e as Error).message}`);
  Deno.exit(1);
}
