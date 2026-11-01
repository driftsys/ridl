// The script of the browser test's page. It connects to the example server
// whose port is the `port` parameter of the page's query string, runs the
// signal half of the end-to-end sequence, and sets `globalThis.result` to a
// promise of what it received: the provenance and the payload, as hex, of
// the signal's first publish and of its publish after the command, the length
// and the byte-for-byte match of each large `fixed`, and the count of discard
// notes.

import { connect, Kind } from "../mod.ts";
import type { Sample } from "../mod.ts";
import * as catalog from "./catalog.ts";
import { waiter } from "./waiter.ts";

/** What the page received, each as `<provenance>:<hex payload>`. */
export interface Result {
  init: string;
  live: string;
  /** Each large `fixed`, by ordinal: `<length>:<match>`. */
  fixed: Record<string, string>;
  /** The count of `discarded` notes. */
  discarded: number;
}

function describe(sample: Sample): string {
  const hex = Array.from(
    sample.payload ?? [],
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");
  return `${sample.provenance}:${hex}`;
}

async function run(): Promise<Result> {
  const port = new URLSearchParams(location.search).get("port");
  if (!port) throw new Error("the page URL has no port parameter");
  const init = waiter<Sample>("init publish", 2000);
  const live = waiter<Sample>("live publish", 2000);
  let discarded = 0;
  const session = await connect(`ws://127.0.0.1:${port}`, {
    catalog: { name: catalog.NAME, hash: catalog.HASH },
    encoding: catalog.ENCODING,
    onPublish(target, sample) {
      if (target.iface !== catalog.IFACE || target.ord !== catalog.SIGNAL) {
        return;
      }
      if (sample.provenance === "init") init.accept(sample);
      if (sample.provenance === "live") live.accept(sample);
    },
    onNote(note) {
      if (note.note === "discarded") discarded++;
    },
  });
  try {
    const fixed: Record<string, string> = {};
    for (const [ord, length] of catalog.FIXED_LENGTHS) {
      const sample = await session.read({
        kind: Kind.Fixed,
        iface: catalog.IFACE,
        ord,
      });
      const expected = catalog.pattern(length);
      const payload = sample.payload;
      const match = payload !== null && payload.length === expected.length &&
        payload.every((b, i) => b === expected[i]);
      fixed[ord] = `${payload?.length ?? 0}:${match}`;
    }
    init.start();
    await session.subscribe({
      kind: Kind.Signal,
      iface: catalog.IFACE,
      ord: catalog.SIGNAL,
    });
    const first = await init.promise;
    // The time limit of the live publish counts from the command.
    live.start();
    await session.command(
      catalog.IFACE,
      catalog.COMMAND,
      Uint8Array.of(0x2a, 0x07),
      { bound: 2000 },
    );
    const second = await live.promise;
    return {
      init: describe(first),
      live: describe(second),
      fixed,
      discarded,
    };
  } finally {
    init.cancel();
    live.cancel();
    session.close();
  }
}

(globalThis as { result?: Promise<Result> }).result = run();
