// The script of the browser test's page. It connects to the example server
// whose port is the `port` parameter of the page's query string, runs the
// signal half of the end-to-end sequence, and sets `globalThis.result` to a
// promise of what it received: the provenance and the payload, as hex, of
// the signal's first publish and of its publish after the command, the length
// and the byte-for-byte match of each large `fixed`, the count and the match
// of the chunks of a streamed reply of 64 elements, the length and match of
// an upload read back through a streamed reply, and the count of discard notes.

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
  /** The streamed reply of 64 elements: `<count>:<match>`. */
  stream: string;
  /** The upload read back through FETCH: `<length>:<match>`. */
  upload: string;
  /**
   * The signal check scenario: `<provenance>:<hex payload>:<hex lastGood>:<rule>`
   * of a good publish, a refused publish, a read after it, and a recovery.
   */
  checked: string[];
  /** The count of `discarded` notes. */
  discarded: number;
}

function hex(bytes: Uint8Array | null): string {
  return Array.from(bytes ?? [], (b) => b.toString(16).padStart(2, "0")).join(
    "",
  );
}

/** A sample as `<provenance>:<payload>:<lastGood>:<rule>`, each part hex or text. */
function describeChecked(sample: Sample): string {
  const detected = sample.detected;
  const rule = detected === undefined
    ? ""
    : typeof detected === "string"
    ? detected
    : detected.rule;
  return `${sample.provenance}:${hex(sample.payload)}:${
    hex(sample.lastGood)
  }:${rule}`;
}

function describe(sample: Sample): string {
  const hex = Array.from(
    sample.payload ?? [],
    (b) => b.toString(16).padStart(2, "0"),
  ).join("");
  return `${sample.provenance}:${hex}`;
}

/**
 * The time limit of each connect, subscribe and read step of the page, in
 * milliseconds. It is far above the time a step takes on a developer machine
 * (the whole run takes about one second), and the five steps together stay
 * below the page limit of the browser test. A step that stalls fails with its
 * own name.
 */
const STEP_BOUND = 4000;

/** Names `stage` in the error of a step that fails, so that a stall shows where it happened. */
async function at<T>(stage: string, step: Promise<T>): Promise<T> {
  try {
    return await step;
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error);
    throw new Error(`${stage}: ${reason}`);
  }
}

async function run(): Promise<Result> {
  const port = new URLSearchParams(location.search).get("port");
  if (!port) throw new Error("the page URL has no port parameter");
  const init = waiter<Sample>("init publish", 2000);
  const live = waiter<Sample>("live publish", 2000);
  let discarded = 0;
  // The signal check scenario waits for each publish through `step`.
  let checking = false;
  let step = waiter<Sample>("checked publish", 2000);
  const session = await at(
    "connect",
    connect(`ws://127.0.0.1:${port}`, {
      bound: STEP_BOUND,
      catalog: { name: catalog.NAME, hash: catalog.HASH },
      encoding: catalog.ENCODING,
      checks: catalog.SIGNAL_CHECKS,
      onPublish(target, sample) {
        if (target.iface !== catalog.IFACE || target.ord !== catalog.SIGNAL) {
          return;
        }
        if (sample.provenance === "init") init.accept(sample);
        if (sample.provenance === "live") live.accept(sample);
        if (checking) step.accept(sample);
      },
      onNote(note) {
        if (note.note === "discarded") discarded++;
      },
    }),
  );
  try {
    const fixed: Record<string, string> = {};
    for (const [ord, length] of catalog.FIXED_LENGTHS) {
      const sample = await at(
        `read of fixed ${ord}`,
        session.read({
          kind: Kind.Fixed,
          iface: catalog.IFACE,
          ord,
        }, { bound: STEP_BOUND }),
      );
      const expected = catalog.pattern(length);
      const payload = sample.payload;
      const match = payload !== null && payload.length === expected.length &&
        payload.every((b, i) => b === expected[i]);
      fixed[ord] = `${payload?.length ?? 0}:${match}`;
    }
    init.start();
    await at(
      "subscribe",
      session.subscribe({
        kind: Kind.Signal,
        iface: catalog.IFACE,
        ord: catalog.SIGNAL,
      }, { bound: STEP_BOUND }),
    );
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
    // The check scenario: the provider sets the signal to each command's
    // argument, and the registered check refuses the one whose first byte is 255.
    checking = true;
    const checked: string[] = [];
    const publishAfter = async (stage: string, args: Uint8Array) => {
      step = waiter<Sample>(`${stage} publish`, 2000);
      step.start();
      await at(
        `${stage} command`,
        session.command(catalog.IFACE, catalog.COMMAND, args, { bound: 2000 }),
      );
      return await at(`${stage} publish`, step.promise);
    };
    checked.push(describeChecked(
      await publishAfter("good", catalog.GOOD_FIRST),
    ));
    checked.push(describeChecked(
      await publishAfter("refused", catalog.REFUSED),
    ));
    checked.push(describeChecked(
      await at(
        "read after refusal",
        session.read({
          kind: Kind.Signal,
          iface: catalog.IFACE,
          ord: catalog.SIGNAL,
        }, { bound: STEP_BOUND }),
      ),
    ));
    checked.push(describeChecked(
      await publishAfter("recovery", catalog.GOOD_SECOND),
    ));
    checking = false;
    step.cancel();
    const elements = await catalog.readElements(
      session.queryStream(
        catalog.IFACE,
        catalog.STREAM_QUERY,
        Uint8Array.of(64),
        { bound: 2000 },
      ),
    );
    const bytes = catalog.pattern(catalog.UPLOAD_LEN);
    const { upload, accepted } = session.commandStream(
      catalog.IFACE,
      catalog.UPLOAD,
      Uint8Array.of(3, 0),
      { bound: 2000 },
    );
    await accepted;
    await catalog.writeAll(upload, bytes, catalog.FETCH_CHUNK);
    const fetched = await catalog.readAll(session.queryStream(
      catalog.IFACE,
      catalog.FETCH,
      Uint8Array.of(3),
      { bound: 2000 },
    ));
    const match = fetched.length === bytes.length &&
      fetched.every((byte, i) => byte === bytes[i]);
    return {
      init: describe(first),
      live: describe(second),
      fixed,
      checked,
      stream: `${elements.count}:${elements.match}`,
      upload: `${fetched.length}:${match}`,
      discarded,
    };
  } finally {
    init.cancel();
    live.cancel();
    step.cancel();
    session.close();
  }
}

(globalThis as { result?: Promise<Result> }).result = run();
