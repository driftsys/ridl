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
// [-40..85], every received value is 1 to 63 steps ahead of the previous one
// on the ring of the 126 values (85 is followed by -40; a repeated value is
// ignored), 85 is reached by such a step, and -40 is seen after 85 (the first
// sample counts for neither bound). Otherwise it exits with a non-zero status
// and names the cause and the stage. One deadline, counted from the start of
// the program, covers every stage including the exit of the server. A server
// that exits or a connection that closes before the end of the check fails the
// program at once. It prints the number of samples it received, the number of
// wraps (a step from 85 to -40 or across it), whether 85 was seen, and whether
// -40 was seen after 85.

import * as flatbuffers from "flatbuffers";
import { connect, Kind } from "../../../packages/ridl-transport-ws/mod.ts";
import { TemperatureBox } from "./generated/veh/cabin.ts";

/** The lowest and the highest value of `Temperature` in cabin.ridl. */
const LOWEST = -40;
const HIGHEST = 85;

/** The number of values in the range of `Temperature`. */
const RING = HIGHEST - LOWEST + 1;

/**
 * The largest forward step between two received values that is not an error.
 * The server publishes forward on the ring of RING values, and the transport
 * sends only the newest value of a signal per tick, so samples between two
 * ticks are skipped and a step larger than one is normal. A forward step of s
 * and a backward step of RING - s give the same difference modulo RING, so only
 * steps up to half the ring can be told apart from a backward move: 63 is
 * RING / 2. A gap of more than 63 publications between two received values
 * therefore fails a correct server. The measured period of the server is about
 * 10 ms (the 7 ms sleep overshoots), so that is a gap of about 630 ms. A step
 * of 0 is a value sent twice, which the transport can do; it is ignored.
 *
 * Seeing 85 reached by a step, and then -40, is what rejects a server that
 * wraps to another value, a server that alternates the bounds, and a decoder
 * that is off by a constant c. The -40 of the first sample does not count,
 * because it comes before any wrap, and neither does an 85 in the first
 * sample, because no step has checked it. With c < 0 the range check fails, or
 * 85 is never seen. With c > 0 the range check fails when 85 + c is received,
 * and otherwise -40 is never seen after 85.
 */
const MAX_STEP = RING / 2;

/**
 * The time from the start of the program to its end, in milliseconds. A correct
 * run needs a sample of 85 and then one of -40, each taken from a window of one
 * value, so the time varies with the phase of the tick. The worst pass measured
 * at the default tick of 20 ms is about 12 s; 30 s leaves margin over it.
 */
const DEADLINE_MS = 30_000;

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
// True when -40 was received after 85 had been received.
let sawLowestAfter = false;
let sawHighest = false;
// The number of steps that crossed from 85 to -40.
let wraps = 0;
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
      // The same value again is a repeated send, not progress.
      if (value === previous) return;
      const step = (value - previous + RING) % RING;
      if (step < 1 || step > MAX_STEP) {
        reject(
          `temperature went from ${previous} to ${value}, a step of ` +
            `${step} on the ring of ${RING} values, not 1 to ${MAX_STEP}`,
        );
        return;
      }
      if (value < previous) wraps++;
      // Only a value reached by a checked step counts as seen.
      if (value === HIGHEST) sawHighest = true;
      if (value === LOWEST && sawHighest) sawLowestAfter = true;
    }
    previous = value;
    if (sawLowestAfter) enough();
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

stage = "waiting for the highest value and the lowest after it";
await done;

// Ends the server: it exits when its standard input reaches the end.
stage = "waiting for the server to exit";
stopping = true;
await server.stdin.close();
const status = await server.status;

console.log(
  `cabin-web: received ${received} sample(s), ${wraps} wrap(s), ` +
    `highest ${HIGHEST} seen: ${sawHighest}, ` +
    `lowest ${LOWEST} seen after it: ${sawLowestAfter}`,
);
if (failure) {
  stage = failureStage;
  fail(failure);
}
if (status.code !== 0) fail(`the server exited with status ${status.code}`);
console.log("cabin-web: ok");
Deno.exit(0);
