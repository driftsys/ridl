import assert from "node:assert/strict";
import { type Browser, launch } from "./browser.ts";
import { type Server, startServer } from "./server.ts";

const PAGE = `${import.meta.dirname}/page.ts`;

const HTML = `<!doctype html>
<meta charset="utf-8">
<title>ridl-transport-ws</title>
<script type="module" src="/page.js"></script>
`;

/** Bundles the page's script for the browser into `out`. */
async function bundle(out: string): Promise<void> {
  const { code, stderr } = await new Deno.Command(Deno.execPath(), {
    args: [
      "bundle",
      "--quiet",
      "--platform=browser",
      "--output",
      out,
      PAGE,
    ],
    stdout: "null",
    stderr: "piped",
  }).output();
  if (code !== 0) {
    throw new Error(
      `deno bundle failed: ${new TextDecoder().decode(stderr)}`,
    );
  }
}

/**
 * The time limit of the whole page run, in milliseconds. The steps of the page
 * that can stall have limits of their own and name themselves in their errors,
 * so this limit is normally reached only by a page that does not load. One CI
 * run took 39 s for this test and a rerun of the same tree took 7 s, while the
 * page run takes about one second on a developer machine.
 */
const PAGE_LIMIT = 30_000;

/** Rejects after `ms` unless `promise` settles first. */
async function limit<T>(promise: Promise<T>, ms: number): Promise<T> {
  let timer: ReturnType<typeof setTimeout> | undefined;
  const late = new Promise<never>((_, reject) => {
    timer = setTimeout(
      () =>
        reject(
          new Error(
            `no result within ${ms} ms: the page did not load, or a step of it with no limit of its own stalled`,
          ),
        ),
      ms,
    );
  });
  try {
    return await Promise.race([promise, late]);
  } finally {
    clearTimeout(timer);
  }
}

Deno.test("a browser page receives a live signal from the Rust server", async () => {
  const dir = await Deno.makeTempDir({ prefix: "ridl-page-" });
  let server: Server | undefined;
  let http: Deno.HttpServer<Deno.NetAddr> | undefined;
  let browser: Browser | undefined;
  // The failure of the test's steps, reported before a failure of the
  // clean-up.
  let failure: { error: unknown } | undefined;
  try {
    const script = `${dir}/page.js`;
    await bundle(script);
    const js = await Deno.readFile(script);
    server = await startServer();
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
    const pageUrl = `http://127.0.0.1:${http.addr.port}/?port=${server.port}`;
    browser = await launch();
    const result = await limit(
      browser.evaluate(pageUrl, "globalThis.result"),
      PAGE_LIMIT,
    );
    assert.deepEqual(result, {
      init: "init:",
      live: "live:2a07",
      fixed: { "7": "65280:true", "8": "16137:true", "9": "16138:true" },
      checked: [
        "live:1001:1001:",
        "invalid::1001:range",
        "invalid::1001:range",
        "live:2002:2002:",
      ],
      stream: "64:true",
      upload: "200000:true",
      discarded: 0,
    });
  } catch (error) {
    failure = { error };
  }
  // Every clean-up step runs even when an earlier one fails, so that no
  // process, listener or directory outlives the test.
  const steps = await Promise.allSettled([
    browser?.close(),
    http?.shutdown(),
    server?.stop(),
    Deno.remove(dir, { recursive: true }),
  ]);
  if (failure) throw failure.error;
  for (const step of steps) {
    if (step.status === "rejected") throw step.reason;
  }
});
