// Reads cabin's `temperature` signal from the WebSocket server in
// ../server and decodes each payload with the code that `flatc` generated
// from the `.fbs` file that `ridl build --emit flatbuffers` wrote.
//
// Usage: deno run -A main.ts <server-executable>
//
// The program starts the server, reads the `port` and `hash` lines it prints,
// attaches with the encoding `flatbuffers`, and subscribes to
// `Cabin.temperature`. It exits with status 0 when it has received at least
// two distinct values and every value is inside the range of `Temperature`.
// Otherwise it exits with a non-zero status and names the cause. It prints
// every distinct value it received, so a reader of the log sees the values
// and not only the verdict.

import * as flatbuffers from "flatbuffers";
import { connect, Kind } from "../../../packages/ridl-transport-ws/mod.ts";
import { TemperatureBox } from "./generated/veh/cabin.ts";

/** The lowest and the highest value of `Temperature` in cabin.ridl. */
const LOWEST = -40;
const HIGHEST = 85;

/** The time the program waits for the values before it fails. */
const TIMEOUT_MS = 10_000;

/** The number of distinct values the program needs to see. */
const REQUIRED_DISTINCT = 2;

// The (interface, ordinal) pair of every interaction the program reads, and
// the function that decodes its payload. Written by hand: the pairs are the
// ones in cabin.ridl, in declaration order, starting at 1.
const decoders = new Map<string, (payload: Uint8Array) => number>([
  // The interface `Signal`, ordinal 1: `temperature`.
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

function fail(message: string): never {
  console.error(`cabin-web: ${message}`);
  Deno.exit(1);
}

const serverPath = Deno.args[0];
if (!serverPath) fail("usage: main.ts <server-executable>");

const server = new Deno.Command(serverPath, {
  stdin: "piped",
  stdout: "piped",
  stderr: "inherit",
}).spawn();

// Reads standard output until both announcement lines have arrived.
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

const values: number[] = [];
const distinct = new Set<number>();
let failure: string | undefined;
let enough!: () => void;
const done = new Promise<void>((resolve) => (enough = resolve));

const session = await connect(`ws://127.0.0.1:${port}`, {
  catalog: { name: "veh.cabin", hash },
  encoding: "flatbuffers",
  onPublish(target, sample) {
    if (!sample.payload) return;
    const decode = decoders.get(key(target.iface, target.ord));
    if (!decode) return;
    const value = decode(sample.payload);
    values.push(value);
    if (value < LOWEST || value > HIGHEST) {
      failure = `temperature ${value} is outside [${LOWEST}..${HIGHEST}]`;
      enough();
      return;
    }
    distinct.add(value);
    if (distinct.size >= REQUIRED_DISTINCT) enough();
  },
});
await session.subscribe({ kind: Kind.Signal, iface: 1, ord: 1 });

const timer = setTimeout(() => {
  failure ??=
    `received ${distinct.size} distinct value(s) in ${TIMEOUT_MS} ms, ` +
    `need ${REQUIRED_DISTINCT}`;
  enough();
}, TIMEOUT_MS);
await done;
clearTimeout(timer);

// Ends the server: it exits when its standard input reaches the end.
await server.stdin.close();
const status = await server.status;

console.log(
  `cabin-web: received ${values.length} sample(s), distinct values: ` +
    `${[...distinct].join(", ")}`,
);
if (failure) fail(failure);
if (status.code !== 0) fail(`the server exited with status ${status.code}`);
console.log("cabin-web: ok");
Deno.exit(0);
