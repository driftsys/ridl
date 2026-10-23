// Finds a Chromium-family browser, starts it headless with a temporary
// profile, and drives it through a minimal client of the Chrome DevTools
// Protocol over Deno's WebSocket.

const MAC_CHROME =
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const PATH_NAMES = [
  "google-chrome",
  "google-chrome-stable",
  "chromium",
  "chromium-browser",
];

/** A running headless browser. */
export interface Browser {
  /**
   * Opens `url` in the browser's page, waits for the page's load event, then
   * evaluates `expression`, awaits it when it is a promise, and resolves with
   * its value as JSON. Rejects with the page's exception when the expression
   * throws or its promise rejects.
   */
  evaluate(url: string, expression: string): Promise<unknown>;
  /** Stops the browser and removes its temporary profile. */
  close(): Promise<void>;
}

async function isFile(path: string): Promise<boolean> {
  try {
    return (await Deno.stat(path)).isFile;
  } catch {
    return false;
  }
}

/**
 * The browser to start: RIDL_BROWSER, then the first of the usual names on
 * PATH, then the Google Chrome application on macOS.
 */
async function findBrowser(): Promise<string> {
  const named = Deno.env.get("RIDL_BROWSER");
  if (named) return named;
  const separator = Deno.build.os === "windows" ? ";" : ":";
  const dirs = (Deno.env.get("PATH") ?? "").split(separator).filter((d) => d);
  for (const name of PATH_NAMES) {
    for (const dir of dirs) {
      const path = `${dir}/${name}`;
      if (await isFile(path)) return path;
    }
  }
  if (await isFile(MAC_CHROME)) return MAC_CHROME;
  throw new Error(
    "no browser found: set RIDL_BROWSER to the path of a Chromium-family " +
      "browser, or install Google Chrome (https://www.google.com/chrome/) " +
      "or Chromium (https://www.chromium.org/getting-involved/download-chromium/)",
  );
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * Reads the DevTools port the browser writes to its profile directory, for
 * up to 30 s. The browser does not write the file atomically, so the port is
 * taken only once its line ends with a newline. When the browser exits or
 * the time runs out, the error names the exit status and ends with the last
 * lines the browser wrote to its standard error.
 */
async function devToolsPort(
  profile: string,
  status: () => Deno.CommandStatus | undefined,
  stderr: () => string,
): Promise<number> {
  const file = `${profile}/DevToolsActivePort`;
  const output = () => {
    const text = stderr().trimEnd();
    return text ? `; its standard error ends with:\n${text}` : "";
  };
  for (let i = 0; i < 300; i++) {
    const exited = status();
    if (exited) {
      throw new Error(
        `the browser exited with code ${exited.code} before it wrote a port ` +
          `to ${file}${output()}`,
      );
    }
    try {
      const text = await Deno.readTextFile(file);
      if (text.includes("\n")) {
        const port = Number(text.split("\n")[0]);
        if (Number.isInteger(port) && port > 0) return port;
      }
    } catch (e) {
      if (!(e instanceof Deno.errors.NotFound)) throw e;
    }
    await delay(100);
  }
  throw new Error(
    `the browser wrote no port to ${file} within 30 s${output()}`,
  );
}

/**
 * Removes the profile directory. The browser's helper processes can still
 * write to it for a short time after the main process has exited, which makes
 * a removal fail with "Directory not empty", so the removal is retried for up
 * to 5 s.
 */
async function removeProfile(profile: string): Promise<void> {
  for (let i = 0;; i++) {
    try {
      await Deno.remove(profile, { recursive: true });
      return;
    } catch (e) {
      if (e instanceof Deno.errors.NotFound) return;
      if (i >= 50) throw e;
    }
    await delay(100);
  }
}

interface Message {
  id?: number;
  method?: string;
  result?: Record<string, unknown>;
  error?: { message: string };
}

interface Waiter {
  settle: (m: Message) => void;
  reject: (e: Error) => void;
}

/**
 * One DevTools Protocol connection to a page target. When the socket closes,
 * every request and every event wait still open rejects, so that no wait
 * outlives the connection.
 */
class Connection {
  #socket: WebSocket;
  #url: string;
  #next = 0;
  #isClosed = false;
  #pending = new Map<number, Waiter>();
  #events = new Set<Waiter>();
  #closed: Promise<void>;

  private constructor(socket: WebSocket, url: string) {
    this.#socket = socket;
    this.#url = url;
    this.#closed = new Promise((resolve) => {
      socket.addEventListener("close", () => {
        this.#isClosed = true;
        const error = this.#closedError();
        const waiters = [...this.#pending.values(), ...this.#events];
        this.#pending.clear();
        this.#events.clear();
        for (const w of waiters) w.reject(error);
        resolve();
      });
    });
    socket.onmessage = (event) => {
      const m = JSON.parse(event.data as string) as Message;
      if (m.id !== undefined) {
        const waiter = this.#pending.get(m.id);
        this.#pending.delete(m.id);
        waiter?.settle(m);
      } else {
        for (const listener of [...this.#events]) listener.settle(m);
      }
    };
  }

  #closedError(): Error {
    return new Error(`the DevTools socket ${this.#url} closed`);
  }

  static open(url: string): Promise<Connection> {
    return new Promise((resolve, reject) => {
      const socket = new WebSocket(url);
      socket.onopen = () => resolve(new Connection(socket, url));
      socket.onerror = () => reject(new Error(`cannot connect to ${url}`));
    });
  }

  /** Sends one command and resolves with its result. */
  send(
    method: string,
    params: Record<string, unknown> = {},
  ): Promise<Record<string, unknown>> {
    const id = ++this.#next;
    return new Promise((resolve, reject) => {
      if (this.#isClosed) return reject(this.#closedError());
      this.#pending.set(id, {
        settle(m) {
          if (m.error) reject(new Error(`${method}: ${m.error.message}`));
          else resolve(m.result ?? {});
        },
        reject,
      });
      this.#socket.send(JSON.stringify({ id, method, params }));
    });
  }

  /** Resolves on the next event named `method`. */
  next(method: string): Promise<void> {
    return new Promise((resolve, reject) => {
      if (this.#isClosed) return reject(this.#closedError());
      const waiter: Waiter = {
        settle: (m) => {
          if (m.method !== method) return;
          this.#events.delete(waiter);
          resolve();
        },
        reject,
      };
      this.#events.add(waiter);
    });
  }

  close(): Promise<void> {
    this.#socket.close();
    return this.#closed;
  }
}

/** Starts the browser headless with a new temporary profile. */
export async function launch(): Promise<Browser> {
  const path = await findBrowser();
  const profile = await Deno.makeTempDir({ prefix: "ridl-browser-" });
  const args = [
    "--headless=new",
    "--remote-debugging-port=0",
    `--user-data-dir=${profile}`,
    "--no-first-run",
    "--no-default-browser-check",
  ];
  if (
    Deno.build.os === "linux" && Deno.env.get("RIDL_BROWSER_NO_SANDBOX") === "1"
  ) {
    args.push("--no-sandbox");
  }
  args.push("about:blank");
  let child: Deno.ChildProcess;
  try {
    child = new Deno.Command(path, { args, stdout: "null", stderr: "piped" })
      .spawn();
  } catch (e) {
    await removeProfile(profile);
    throw e;
  }
  // The standard error is read for as long as the browser runs, so that the
  // pipe never fills; only the last 4 KiB are kept, for a launch error.
  let stderr = "";
  const stderrDone = (async () => {
    for await (
      const chunk of child.stderr.pipeThrough(new TextDecoderStream())
    ) {
      stderr = (stderr + chunk).slice(-4096);
    }
  })();
  let exited: Deno.CommandStatus | undefined;
  child.status.then((s) => exited = s);
  const close = async () => {
    try {
      child.kill();
    } catch {
      // The browser has already exited.
    }
    await child.status;
    await stderrDone;
    await removeProfile(profile);
  };
  let port: number;
  try {
    port = await devToolsPort(profile, () => exited, () => stderr);
  } catch (e) {
    await close();
    throw e;
  }

  return {
    async evaluate(url, expression) {
      const response = await fetch(`http://127.0.0.1:${port}/json/list`);
      const targets = await response.json() as {
        type: string;
        webSocketDebuggerUrl: string;
      }[];
      const page = targets.find((t) => t.type === "page");
      if (!page) throw new Error("the browser has no page target");
      const cdp = await Connection.open(page.webSocketDebuggerUrl);
      try {
        await cdp.send("Page.enable");
        const loaded = cdp.next("Page.loadEventFired");
        const navigated = await cdp.send("Page.navigate", { url });
        if (navigated.errorText) {
          throw new Error(`cannot open ${url}: ${navigated.errorText}`);
        }
        await loaded;
        const evaluated = await cdp.send("Runtime.evaluate", {
          expression,
          awaitPromise: true,
          returnByValue: true,
        });
        const details = evaluated.exceptionDetails as
          | { text: string; exception?: { description?: string } }
          | undefined;
        if (details) {
          throw new Error(
            `the page threw: ${details.exception?.description ?? details.text}`,
          );
        }
        return (evaluated.result as { value?: unknown }).value;
      } finally {
        await cdp.close();
      }
    },
    close,
  };
}
