// Starts the cabin server of ../server and reads the `port` and `hash` lines
// it prints. main.ts and browser.ts both use it.

/** A started cabin server. */
export interface CabinServer {
  /** The port the server listens on, on 127.0.0.1. */
  port: number;
  /** The catalog hash the server announced, as 64 lowercase hex digits. */
  hash: string;
  /** Resolves when the server exits, for whatever reason. */
  status: Promise<Deno.CommandStatus>;
  /** Kills the server; does nothing when it has already exited. */
  kill(): void;
  /**
   * Closes the server's standard input, which makes it exit, and resolves
   * with its exit status.
   */
  stop(): Promise<Deno.CommandStatus>;
}

/**
 * Starts the server executable at `path` and resolves once it has printed
 * its port and its hash. Rejects, naming `path`, when the executable cannot
 * be started or ends before it prints both lines.
 */
export async function startServer(path: string): Promise<CabinServer> {
  let child: Deno.ChildProcess;
  try {
    child = new Deno.Command(path, {
      stdin: "piped",
      stdout: "piped",
      stderr: "inherit",
    }).spawn();
  } catch (e) {
    throw new Error(
      `cannot start the server ${path}: ${(e as Error).message}`,
    );
  }
  const kill = () => {
    try {
      child.kill();
    } catch {
      // The server has already exited.
    }
  };
  const output = child.stdout.pipeThrough(new TextDecoderStream())
    .getReader();
  let announced = "";
  while (!/^hash [0-9a-f]{64}$/m.test(announced)) {
    const { value, done } = await output.read();
    if (done) {
      kill();
      await child.status;
      throw new Error(
        `the server ${path} ended before it printed its port and hash`,
      );
    }
    announced += value;
  }
  const port = Number(/^port (\d+)$/m.exec(announced)?.[1]);
  const hash = /^hash ([0-9a-f]{64})$/m.exec(announced)![1];
  return {
    port,
    hash,
    status: child.status,
    kill,
    async stop() {
      await child.stdin.close();
      return await child.status;
    },
  };
}
