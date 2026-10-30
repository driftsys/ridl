// Reads cabin's `temperature` signal from the WebSocket server in
// ../server and decodes each payload with the codec module that
// `ridl build --emit typescript` wrote, generated/veh.cabin.codec.ts, which
// calls the code `flatc` generated from the `.fbs` file. The interface number
// and the ordinal of the signal come from the same module's `Cabin` table.
// Before it decodes a payload, it verifies the payload through the cabin wasm
// codec, generated/veh_cabin.wasm, loaded with packages/ridl-runtime.
//
// Usage: deno run -A main.ts <server-executable>
//
// The program starts the server, reads the `port` and `hash` lines it prints,
// fails when the announced hash is not the generated `CATALOG_HASH`, loads the
// wasm codec with the generated hash, payload types and rules, runs the codec
// self-test (20
// is accepted, -41 and 86 are rejected with the range rule), attaches with the
// encoding `flatbuffers`, and subscribes to
// `Cabin.temperature`. The server steps the value through the whole range of
// `Temperature` and starts again at the lowest value. The transport may
// coalesce samples (docs/design/ridl-transport-ws.md), so the program does not
// expect every step. It exits with status 0 only if every value is inside
// [-40..85], every received value is 1 to 63 steps ahead of the previous one
// on the ring of the 126 values (85 is followed by -40; a repeated value is
// ignored), 85 is reached by such a step, and -40 is seen after 85 (the first
// sample counts for neither bound). Otherwise it exits with a non-zero status
// and names the cause and the stage. One deadline, counted from the start of
// the program, covers every stage including the exit of the server. A server
// that exits or a connection that closes before the end of the check fails the
// program at once. It prints the number of samples it received, the number of
// wraps (a step from 85 to -40 or across it), whether 85 was seen, and whether
// -40 was seen after 85. A payload the codec rejects fails the program with
// the cause.

import { type Codec, loadCodec } from "../../../packages/ridl-runtime/mod.ts";
import { connect, Kind } from "../../../packages/ridl-transport-ws/mod.ts";
import {
  hexFromHash,
  HIGHEST,
  isPass,
  LOWEST,
  RANGE_RULE,
  selfTest,
  TemperatureCheck,
  verifyPayload,
} from "./check.ts";
import {
  Cabin,
  CATALOG_HASH,
  CATALOG_NAME,
  Horn,
  PAYLOAD_TYPES,
  RULES,
} from "./generated/veh.cabin.codec.ts";
import { type CabinServer, startServer } from "./server.ts";

/**
 * The time from the start of the program to its end, in milliseconds. A correct
 * run needs a sample of 85 and then one of -40, each taken from a window of one
 * value, so the time varies with the phase of the tick. The worst pass measured
 * at the default tick of 20 ms is about 12 s; 30 s leaves margin over it.
 */
const DEADLINE_MS = 30_000;

// The stage the program is in, named in the failure message.
let stage = "starting the server";
// The server, once it has announced its port and hash.
let server: CabinServer | undefined;
// Kills the server; set as soon as the server is spawned.
let killServer: (() => void) | undefined;
// Set when the program has begun to stop the server on purpose.
let stopping = false;

function fail(message: string): never {
  console.error(`cabin-web: ${message} (stage: ${stage})`);
  killServer?.();
  Deno.exit(1);
}

setTimeout(
  () => fail(`the deadline of ${DEADLINE_MS} ms expired`),
  DEADLINE_MS,
);

const serverPath = Deno.args[0];
if (!serverPath) fail("usage: main.ts <server-executable>");

// The module is read before the server starts; it is loaded once the server
// has announced the catalog hash.
stage = "reading the codec";
let wasm: Uint8Array;
try {
  wasm = await Deno.readFile(
    new URL("./generated/veh_cabin.wasm", import.meta.url),
  );
} catch (e) {
  fail(`loading the codec: ${(e as Error).message}`);
}

// The announcement of the server is read inside `startServer`.
stage = "reading the announcement of the server";
try {
  server = await startServer(serverPath, (kill) => (killServer = kill));
} catch (e) {
  fail((e as Error).message);
}
const { port } = server;
void server.status.then((status) => {
  if (!stopping) fail(`the server exited with status ${status.code}`);
});
stage = "comparing the catalog hash";
const generatedHex = hexFromHash(CATALOG_HASH);
if (server.hash !== generatedHex) {
  fail(
    `the server announces catalog hash ${server.hash}, ` +
      `the generated table has ${generatedHex}`,
  );
}

stage = "loading the codec";
let codec: Codec;
try {
  codec = await loadCodec(wasm, {
    catalogHash: CATALOG_HASH,
    payloadTypes: PAYLOAD_TYPES,
    ruleText: RULES,
  });
} catch (e) {
  fail(`loading the codec: ${(e as Error).message}`);
}
stage = "running the self-test";
const selfTestFailure = selfTest(codec);
if (selfTestFailure) fail(`self-test: ${selfTestFailure}`);

const check = new TemperatureCheck();
let failure: string | undefined;
// The stage in which the value that caused `failure` arrived.
let failureStage = "";
let enough!: () => void;
const done = new Promise<void>((resolve) => (enough = resolve));

stage = "connecting";
const session = await connect(`ws://127.0.0.1:${port}`, {
  catalog: { name: CATALOG_NAME, hash: CATALOG_HASH },
  encoding: "flatbuffers",
  // The generated tables give the session the limit of each member a
// `[limits]` entry raises; cabin has none, so every member keeps the
// default ceiling of its kind.
  interfaces: [Cabin, Horn],
  onPublish(target, sample) {
    if (!sample.payload || failure) return;
    if (
      target.iface !== Cabin.temperature.iface ||
      target.ord !== Cabin.temperature.ord
    ) return;
    let verdict;
    try {
      const rejected = verifyPayload(codec, sample.payload);
      verdict = rejected
        ? { kind: "fail" as const, cause: rejected }
        : check.accept(Cabin.temperature.payload.decode(sample.payload));
    } catch (e) {
      fail(`verifying the payload: ${(e as Error).message}`);
    }
    if (verdict.kind === "fail") {
      failure = verdict.cause;
      failureStage = stage;
      enough();
    } else if (verdict.kind === "pass") {
      enough();
    }
  },
});
void session.closed.then(async (ended) => {
  if (stopping) return;
  // A server that exited closes the connection too. Wait briefly for its exit
  // status, so that a failure of the server is reported before the closure.
  const exited = await Promise.race([
    server!.status,
    new Promise<undefined>((resolve) =>
      setTimeout(() => resolve(undefined), 500)
    ),
  ]);
  if (exited && exited.code !== 0) {
    fail(`the server exited with status ${exited.code}`);
  }
  fail(`the connection closed (code ${ended.code})`);
});
stage = "subscribing";
await session.subscribe({
  kind: Kind.Signal,
  iface: Cabin.temperature.iface,
  ord: Cabin.temperature.ord,
});

stage = "waiting for the highest value and the lowest after it";
await done;

// Ends the server: it exits when its standard input reaches the end.
stage = "waiting for the server to exit";
stopping = true;
const status = await server.stop();

console.log(
  `cabin-web: received ${check.progress.received} sample(s), ` +
    `${check.progress.wraps} wrap(s), ` +
    `highest ${HIGHEST} seen: ${check.progress.sawHighest}, ` +
    `lowest ${LOWEST} seen after it: ${check.progress.sawLowestAfter}`,
);
if (failure) {
  stage = failureStage;
  fail(failure);
}
if (status.code !== 0) fail(`the server exited with status ${status.code}`);
if (!isPass(check.progress)) {
  fail(`the check ended without seeing ${LOWEST} after ${HIGHEST}`);
}
console.log(
  `cabin-web: ok, self-test rejected ${LOWEST - 1} and ${HIGHEST + 1} ` +
    `with rule ${RANGE_RULE}`,
);
Deno.exit(0);
