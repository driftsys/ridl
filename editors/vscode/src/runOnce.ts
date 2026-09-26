// A callback that runs at most once, however many times it is called, and
// from however many call sites. Pure — no `vscode` import — so it is
// unit-tested directly, unlike the code that uses it in `extension.ts`.

/** Returns a function that calls `perform` the first time it is called, and does nothing on every later call. */
export function runOnce(perform: () => void): () => void {
  let done = false;
  return () => {
    if (done) return;
    done = true;
    perform();
  };
}
