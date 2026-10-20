// Starts the example WebSocket server of the ridl-transport-ws crate for one
// test. The binary is named by the environment variable
// RIDL_TRANSPORT_WS_SERVER.

const VARIABLE = "RIDL_TRANSPORT_WS_SERVER";

/** A running example server. */
export interface Server {
  /** The TCP port the server listens on, on 127.0.0.1. */
  port: number;
  /** Closes the server's standard input, which ends it, and waits for its exit. */
  stop(): Promise<void>;
}

/**
 * Starts the example server and resolves once it has printed its port.
 * Throws when RIDL_TRANSPORT_WS_SERVER is not set.
 */
export async function startServer(): Promise<Server> {
  const path = Deno.env.get(VARIABLE);
  if (!path) {
    throw new Error(
      `${VARIABLE} is not set: build the server with ` +
        "`cargo build -p ridl-transport-ws --example loopback_server` and " +
        `set ${VARIABLE} to the absolute path of ` +
        "target/debug/examples/loopback_server",
    );
  }
  const child = new Deno.Command(path, {
    stdin: "piped",
    stdout: "piped",
    stderr: "inherit",
  }).spawn();
  const stdin = child.stdin.getWriter();
  const lines = child.stdout.pipeThrough(new TextDecoderStream()).getReader();
  const stop = async () => {
    await stdin.close().catch(() => {});
    await lines.cancel().catch(() => {});
    await child.status;
  };
  try {
    let text = "";
    while (!text.includes("\n")) {
      const { value, done } = await lines.read();
      if (done) throw new Error(`${path} exited before printing its port`);
      text += value;
    }
    const match = /^port (\d+)\n/.exec(text);
    if (!match) throw new Error(`${path} printed ${JSON.stringify(text)}`);
    return { port: Number(match[1]), stop };
  } catch (e) {
    await stop();
    throw e;
  }
}
