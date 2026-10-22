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
// expect every step. It exits with status 0 when it has received both the
// lowest and the highest value, every value is inside the range, and the
// value has decreased at least once, which is the restart. A server that does
// not restart, or that publishes an offset value, fails. Otherwise it exits with a
// non-zero status and names the cause and the stage. One deadline, counted
// from the start of the program, covers every stage including the exit of the
// server. A server that exits or a connection that closes before the end of
// the check fails the program at once. It prints how many samples it received.

import * as flatbuffers from "flatbuffers";
import { connect, Kind } from "../../../packages/ridl-transport-ws/mod.ts";
import { TemperatureBox } from "./generated/veh/cabin.ts";

/** The lowest and the highest value of `Temperature` in cabin.ridl. */
const LOWEST = -40;
const HIGHEST = 85;

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
let sawLowest = false;
let sawHighest = false;
let restarts = 0;
let failure: string | undefined;
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
    if (value < LOWEST || value > HIGHEST) {
      failure = `temperature ${value} is outside [${LOWEST}..${HIGHEST}]`;
      enough();
      return;
    }
    // A decrease is the restart after the highest value. Samples may be
    // skipped, so a step larger than one is not an error.
    if (previous !== undefined && value < previous) restarts++;
    previous = value;
    if (value === LOWEST) sawLowest = true;
    if (value === HIGHEST) sawHighest = true;
    if (sawLowest && sawHighest && restarts > 0) enough();
  },
});
void session.closed.then((ended) => {
  if (!stopping) fail(`the connection closed (code ${ended.code})`);
});
stage = "subscribing";
await session.subscribe({ kind: Kind.Signal, iface: 1, ord: 1 });

stage = "waiting for the lowest and the highest value";
await done;

// Ends the server: it exits when its standard input reaches the end.
stage = "waiting for the server to exit";
stopping = true;
await server.stdin.close();
const status = await server.status;

console.log(
  `cabin-web: received ${received} sample(s), ${restarts} restart(s), ` +
    `lowest ${LOWEST} seen: ${sawLowest}, highest ${HIGHEST} seen: ${sawHighest}`,
);
if (failure) fail(failure);
if (status.code !== 0) fail(`the server exited with status ${status.code}`);
console.log("cabin-web: ok");
Deno.exit(0);
