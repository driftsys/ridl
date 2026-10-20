// The script of the browser test's page. It connects to the example server
// whose port is the `port` parameter of the page's query string, runs the
// signal half of the end-to-end sequence, and sets `globalThis.result` to a
// promise of what it received: the provenance and the payload, as hex, of
// the signal's first publish and of its publish after the command.

import { connect, Kind } from "../mod.ts";
import type { Sample } from "../mod.ts";
import * as catalog from "./catalog.ts";

/** What the page received, each as `<provenance>:<hex payload>`. */
export interface Result {
  init: string;
  live: string;
}

function describe(sample: Sample): string {
  const hex = Array.from(
    sample.payload ?? [],
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");
  return `${sample.provenance}:${hex}`;
}

/** Resolves with the next sample `next` is given, or rejects after `ms`. */
function within(ms: number, what: string) {
  let give!: (sample: Sample) => void;
  const promise = new Promise<Sample>((resolve, reject) => {
    const timer = setTimeout(
      () => reject(new Error(`no ${what} within ${ms} ms`)),
      ms,
    );
    give = (sample) => {
      clearTimeout(timer);
      resolve(sample);
    };
  });
  return { promise, give };
}

async function run(): Promise<Result> {
  const port = new URLSearchParams(location.search).get("port");
  if (!port) throw new Error("the page URL has no port parameter");
  const init = within(2000, "init publish");
  const live = within(2000, "live publish");
  const session = await connect(`ws://127.0.0.1:${port}`, {
    catalog: { name: catalog.NAME, hash: catalog.HASH },
    encoding: catalog.ENCODING,
    onPublish(target, sample) {
      if (target.iface !== catalog.IFACE || target.ord !== catalog.SIGNAL) {
        return;
      }
      if (sample.provenance === "init") init.give(sample);
      if (sample.provenance === "live") live.give(sample);
    },
  });
  try {
    await session.subscribe({
      kind: Kind.Signal,
      iface: catalog.IFACE,
      ord: catalog.SIGNAL,
    });
    const first = await init.promise;
    await session.command(
      catalog.IFACE,
      catalog.COMMAND,
      Uint8Array.of(0x2a, 0x07),
      { bound: 2000 },
    );
    const second = await live.promise;
    return { init: describe(first), live: describe(second) };
  } finally {
    session.close();
  }
}

(globalThis as { result?: Promise<Result> }).result = run();
