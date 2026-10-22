// Reads cabin's `temperature` signal from the WebSocket server in
// ../server and decodes each payload with the code that `flatc` generated
// from the `.fbs` file that `ridl build --emit flatbuffers` wrote.
//
// Usage: deno run -A main.ts <server-executable>
//
// The program starts the server, reads the `port` and `hash` lines it prints,
// attaches with the encoding `flatbuffers`, and subscribes to
// `Cabin.temperature`. The server steps the value through the whole range of
// `Temperature` and starts again at the lowest value. The transport may
// coalesce samples (docs/design/ridl-transport-ws.md), so the program does not
// expect every step. It exits with status 0 only if every value is inside
// [-40..85], a decrease lands at or below -35 at least once (a restart), and
// 85 is seen. Any other decrease fails, and so do two decreases in a row and
// an increase larger than MAX_INCREASE. Otherwise it exits with a non-zero status and names the
// cause and the stage. One deadline, counted
// from the start of the program, covers every stage including the exit of the
// server. A server that exits or a connection that closes before the end of
// the check fails the program at once. It prints how many samples it received.

import * as flatbuffers from "flatbuffers";
import { connect, Kind } from "../../../packages/ridl-transport-ws/mod.ts";
import { TemperatureBox } from "./generated/veh/cabin.ts";

/** The lowest and the highest value of `Temperature` in cabin.ridl. */
const LOWEST = -40;
const HIGHEST = 85;

/**
 * The largest increase between two received values that is not an error. The
 * server publishes every 10 ms, and the transport sends the newest value of a
 * signal once per tick, so samples published between two ticks are coalesced
 * (docs/design/ridl-transport-ws.md). A step larger than one is therefore
 * normal. 20 is generous: it is about 200 ms of publications, which is many
 * ticks, and a server that jumps between the bounds exceeds it.
 */
const MAX_INCREASE = 20;

/**
 * The highest value a restart may land on. The server wraps from 85 to -40, but
 * the transport sends only the newest value per tick, so the client may miss
 * the first few values after a wrap and see a later one.
 */
const RESTART_CEILING = -35;

/** The time from the start of the program to its end, in milliseconds. */
const DEADLINE_MS = 20_000;

// The (interface, ordinal) pair of every interaction the program reads, and
// the function that decodes its payload. Written by hand: the pairs are the
// ones in cabin.ridl, in declaration order, starting at 1.
const decoders = new Map<string, (payload: Uint8Array) => number>([
  // Interface 1 is `Cabin`, and its interaction 1 is the signal
  // `temperature`.
  [
    key(1, 1),
    (payload) =>
      TemperatureBox.getRootAsTemperatureBox(
        new flatbuffers.ByteBuffer(payload),
      ).value(),
  ],
]);

function key(iface: number, ord: number): string {
  return `${iface}/${ord}`;
}

// The stage the program is in, named in the failure message.
let stage = "starting the server";
// The server process, once it has started.
const started: Deno.ChildProcess[] = [];
// Set when the program has begun to stop the server on purpose.
let stopping = false;

function fail(message: string): never {
  console.error(`cabin-web: ${message} (stage: ${stage})`);
  try {
    for (const process of started) process.kill();
  } catch {
    // The server has already ended.
  }
  Deno.exit(1);
}

setTimeout(
  () => fail(`the deadline of ${DEADLINE_MS} ms expired`),
  DEADLINE_MS,
);

const serverPath = Deno.args[0];
if (!serverPath) fail("usage: main.ts <server-executable>");

const server = new Deno.Command(serverPath, {
  stdin: "piped",
  stdout: "piped",
  stderr: "inherit",
}).spawn();
started.push(server);
void server.status.then((status) => {
  if (!stopping) fail(`the server exited with status ${status.code}`);
});

// Reads standard output until both announcement lines have arrived.
stage = "reading the announcement of the server";
const output = server.stdout.pipeThrough(new TextDecoderStream()).getReader();
let announced = "";
while (!/^hash [0-9a-f]{64}$/m.test(announced)) {
  const { value, done } = await output.read();
  if (done) fail(`the server ended before it printed its port and hash`);
  announced += value;
}
const port = Number(/^port (\d+)$/m.exec(announced)?.[1]);
const hex = /^hash ([0-9a-f]{64})$/m.exec(announced)?.[1] ?? "";
const hash = Uint8Array.from(hex.match(/../g)!.map((h) => parseInt(h, 16)));

let received = 0;
let previous: number | undefined;
let sawHighest = false;
// True when the previous step was a decrease.
let decreased = false;
let restarts = 0;
let failure: string | undefined;
// The stage in which the value that caused `failure` arrived.
let failureStage = "";
let enough!: () => void;
const done = new Promise<void>((resolve) => (enough = resolve));

stage = "connecting";
const session = await connect(`ws://127.0.0.1:${port}`, {
  catalog: { name: "veh.cabin", hash },
  encoding: "flatbuffers",
  onPublish(target, sample) {
    if (!sample.payload || failure) return;
    const decode = decoders.get(key(target.iface, target.ord));
    if (!decode) return;
    const value = decode(sample.payload);
    received++;
    const reject = (cause: string) => {
      failure = cause;
      failureStage = stage;
      enough();
    };
    if (value < LOWEST || value > HIGHEST) {
      reject(`temperature ${value} is outside [${LOWEST}..${HIGHEST}]`);
      return;
    }
    if (previous !== undefined) {
      if (value < previous) {
        // A decrease is valid only as the restart, at or below the ceiling,
        // and never twice in a row.
        if (value > RESTART_CEILING) {
          reject(
            `temperature fell from ${previous} to ${value}, ` +
              `not to ${RESTART_CEILING} or lower`,
          );
          return;
        }
        if (decreased) {
          reject(`temperature fell twice in a row, to ${value}`);
          return;
        }
        restarts++;
        decreased = true;
      } else {
        decreased = false;
        if (value - previous > MAX_INCREASE) {
          reject(
            `temperature rose from ${previous} to ${value}, ` +
              `more than ${MAX_INCREASE}`,
          );
          return;
        }
      }
    }
    previous = value;
    if (value === HIGHEST) sawHighest = true;
    if (sawHighest && restarts > 0) enough();
  },
});
void session.closed.then(async (ended) => {
  if (stopping) return;
  // A server that exited closes the connection too. Wait briefly for its exit
  // status, so that a failure of the server is reported before the closure.
  const exited = await Promise.race([
    server.status,
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
await session.subscribe({ kind: Kind.Signal, iface: 1, ord: 1 });

stage = "waiting for a restart and the highest value";
await done;

// Ends the server: it exits when its standard input reaches the end.
stage = "waiting for the server to exit";
stopping = true;
await server.stdin.close();
const status = await server.status;

console.log(
  `cabin-web: received ${received} sample(s), ${restarts} restart(s), ` +
    `highest ${HIGHEST} seen: ${sawHighest}`,
);
if (failure) {
  stage = failureStage;
  fail(failure);
}
if (status.code !== 0) fail(`the server exited with status ${status.code}`);
console.log("cabin-web: ok");
Deno.exit(0);
