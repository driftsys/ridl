// The script of the page that browser.ts opens. It reads the server's `port`
// and the catalog `hash` (64 hex digits) from the page's query string,
// attaches with the encoding `flatbuffers`, subscribes to `Cabin.temperature`,
// and feeds each decoded value to the check of check.ts. It sets
// `globalThis.result` to a promise that resolves with the check's progress on
// the first `pass` verdict, and rejects with the cause on a `fail` verdict or
// when the connection closes first.

import { connect, Kind } from "../../../packages/ridl-transport-ws/mod.ts";
import {
  decodeTemperature,
  hashFromHex,
  type Progress,
  TEMPERATURE,
  TemperatureCheck,
} from "./check.ts";

async function run(): Promise<Progress> {
  const query = new URLSearchParams(location.search);
  const port = query.get("port");
  if (!port) throw new Error("the page URL has no port parameter");
  const hex = query.get("hash") ?? "";
  if (!/^[0-9a-f]{64}$/.test(hex)) {
    throw new Error("the page URL has no hash parameter of 64 hex digits");
  }
  const hash = hashFromHex(hex);

  const check = new TemperatureCheck();
  // Set once the check has given its first `pass` or `fail`; later values
  // are not fed to the check.
  let settled = false;
  let resolve!: (progress: Progress) => void;
  let reject!: (error: Error) => void;
  const verdict = new Promise<Progress>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  const session = await connect(`ws://127.0.0.1:${port}`, {
    catalog: { name: "veh.cabin", hash },
    encoding: "flatbuffers",
    onPublish(target, sample) {
      if (settled || !sample.payload) return;
      if (
        target.iface !== TEMPERATURE.iface || target.ord !== TEMPERATURE.ord
      ) return;
      const outcome = check.accept(decodeTemperature(sample.payload));
      if (outcome.kind === "pass") {
        settled = true;
        resolve({ ...check.progress });
      } else if (outcome.kind === "fail") {
        settled = true;
        reject(new Error(outcome.cause));
      }
    },
  });
  void session.closed.then((ended) => {
    if (settled) return;
    settled = true;
    reject(new Error(`the connection closed (code ${ended.code})`));
  });
  try {
    await session.subscribe({
      kind: Kind.Signal,
      iface: TEMPERATURE.iface,
      ord: TEMPERATURE.ord,
    });
    return await verdict;
  } finally {
    settled = true;
    session.close();
  }
}

(globalThis as { result?: Promise<Progress> }).result = run();
