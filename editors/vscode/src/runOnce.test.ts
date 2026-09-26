import { strict as assert } from "node:assert";
import { test } from "node:test";
import { runOnce } from "./runOnce";

test("runOnce calls the callback on the first call", () => {
  let calls = 0;
  const run = runOnce(() => {
    calls += 1;
  });
  run();
  assert.equal(calls, 1);
});

test("runOnce does not call the callback again on a later call", () => {
  let calls = 0;
  const run = runOnce(() => {
    calls += 1;
  });
  run();
  run();
  run();
  assert.equal(calls, 1);
});

test("runOnce guards across two different call sites sharing the same returned function", () => {
  // Models extension.ts: startClientIfNeeded and the ridl.serverPath restart
  // handler both call the same runOnce-wrapped offer.
  let calls = 0;
  const offer = runOnce(() => {
    calls += 1;
  });
  const fromDocumentOpen = () => offer();
  const fromRestart = () => offer();
  fromDocumentOpen();
  fromRestart();
  assert.equal(calls, 1);
});
