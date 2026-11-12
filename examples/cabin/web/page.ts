// The script of the page that browser.ts opens. It reads the server's `port`
// and the catalog `hash` (64 hex digits) that the server announced from the
// page's query string, rejects when that hash is not the `CATALOG_HASH` of the
// generated codec module, fetches the cabin wasm codec from /veh_cabin.wasm
// and loads it with the generated hash, payload types and rules, runs the
// codec self-test of check.ts, attaches with the encoding `flatbuffers`
// through a session with a bound of 10 s on `connect` and `subscribe`, and
// binds the generated `CabinClient` to it. Through the client it sends
// `setLevel` with 30 and with 51, calls `average` and rejects unless the reply
// is 40 (the mean, rounded down), then subscribes to the `temperature` signal and watches it. The client
// verifies each payload through the codec and decodes it; the page feeds each
// decoded value to the check of check.ts. It sets `globalThis.result` to a
// promise that resolves with the check's progress on the first `pass` verdict,
// and rejects with the cause when the codec cannot be loaded, the self-test
// fails, a call fails or replies another value, the client refuses or the
// session check throws on a payload, the check gives a `fail` verdict, or the
// connection closes first.

import { type Codec, loadCodec } from "../../../packages/ridl-runtime/mod.ts";
import { connect } from "../../../packages/ridl-transport-ws/mod.ts";
import {
  exerciseCalls,
  hexFromHash,
  type Progress,
  selfTest,
  sessionOptions,
  subscribeTemperature,
  TemperatureCheck,
} from "./check.ts";
import {
  CabinClient,
  CATALOG_HASH,
  PAYLOAD_TYPES,
  RULES,
} from "./generated/veh.cabin.codec.ts";

async function run(): Promise<Progress> {
  const query = new URLSearchParams(location.search);
  const port = query.get("port");
  if (!port) throw new Error("the page URL has no port parameter");
  const hex = query.get("hash") ?? "";
  if (!/^[0-9a-f]{64}$/.test(hex)) {
    throw new Error("the page URL has no hash parameter of 64 hex digits");
  }
  const generatedHex = hexFromHash(CATALOG_HASH);
  if (hex !== generatedHex) {
    throw new Error(
      `the server announces catalog hash ${hex}, ` +
        `the generated table has ${generatedHex}`,
    );
  }

  let codec: Codec;
  try {
    codec = await loadCodec(fetch("/veh_cabin.wasm"), {
      catalogHash: CATALOG_HASH,
      payloadTypes: PAYLOAD_TYPES,
      ruleText: RULES,
    });
  } catch (e) {
    throw new Error(`loading the codec: ${(e as Error).message}`);
  }
  const selfTestFailure = selfTest(codec);
  if (selfTestFailure) throw new Error(`self-test: ${selfTestFailure}`);

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
    ...sessionOptions(),
    onNote(note) {
      if (note.note === "check-failed" && !settled) {
        settled = true;
        reject(new Error(`the signal check threw: ${String(note.error)}`));
      }
    },
  });
  const cabin = CabinClient.bind(session, codec);
  cabin.watchTemperature((sample) => {
    if (settled) return;
    // A payload the client refuses arrives as an invalid sample with
    // `detected` set; one that is not live carries no new value.
    if (sample.detected !== undefined) {
      settled = true;
      reject(
        new Error(
          `the client refused a payload: ${JSON.stringify(sample.detected)}`,
        ),
      );
      return;
    }
    if (sample.provenance !== "live") return;
    const outcome = check.accept(sample.value);
    if (outcome.kind === "pass") {
      settled = true;
      resolve({ ...check.progress });
    } else if (outcome.kind === "fail") {
      settled = true;
      reject(new Error(outcome.cause));
    }
  });
  void session.closed.then((ended) => {
    if (settled) return;
    settled = true;
    reject(new Error(`the connection closed (code ${ended.code})`));
  });
  try {
    const cause = await exerciseCalls(cabin);
    if (cause) throw new Error(cause);
    await subscribeTemperature(cabin);
    return await verdict;
  } finally {
    settled = true;
    session.close();
  }
}

(globalThis as { result?: Promise<Progress> }).result = run();
