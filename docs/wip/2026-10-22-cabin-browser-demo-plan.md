# Cabin Browser Demo Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** A headless browser page receives cabin's `temperature` signal from the
cabin WebSocket server, decodes it with the `flatc` code, and passes the same
check as the Deno demo.

**Architecture:** The check in `examples/cabin/web/main.ts` moves to a module
with no Deno API, `check.ts`, which both the Deno program and a new browser page
use. A Deno runner starts the server, bundles the page with `deno bundle`,
serves it from `Deno.serve`, drives a headless browser with the existing
`packages/ridl-transport-ws/e2e/browser.ts`, and reads the page's result. A new
recipe `just cabin-browser-demo` runs it; it is not a member of `build`, like
`ts-browser-e2e`, and CI runs it in the job that runs `ts-browser-e2e`.

**Tech Stack:** Deno (version in `.deno-version`), `flatc` (version in
`.flatc-version`), the `flatbuffers` npm package already in
`examples/cabin/web/deno.json`, Chromium-family browser over the Chrome DevTools
Protocol.

**Spec:** `docs/wip/2026-10-21-ts-wasm-codec-design.md` (D-7, slice 4: "the
end-to-end proof of D-7 in Deno, then in a browser, wired into `just` and CI").
The wasm verify half of D-7 is step 4 of the web-session plan, not this plan.

## Global Constraints

- The page and `check.ts` use no `Deno` API; they run in a browser.
- The public TypeScript API rule: plain objects only. `flatc` accessors stay
  inside the decoder function.
- The pass condition is unchanged from #1022: values in `[-40..85]`, each new
  value 1 to 63 steps ahead on the ring of 126 values, repeats ignored, 85
  reached by a checked step, then -40.
- Prose is plain and literal; no story ids in `examples/`
  (`just
  story-id-check`).
- A gate command lives only in the `justfile`; the workflow runs
  `just cabin-browser-demo` (ADR-0009).

## Review Focus

1. A page that throws before it sets its result: the runner must fail with the
   page's exception, not wait for the deadline. Task 2 tests it by the `limit()`
   pattern plus `evaluate` rejecting.
2. The server exits early: the runner fails and names the server status. Task 2
   checks the status before reporting a pass.
3. A browser that is not found: the recipe fails with the same message as
   `ts-browser-e2e`. Task 2 copies the lookup.
4. The checker reports a pass before 85 was reached by a step (first sample is
   85). Task 1 pins it with a unit test.
5. Leaked processes: the browser and the server are stopped on every path. Task
   2 uses `try`/`finally`.

## Delegated decisions

- P-1. `check.ts` is a pure state machine; Deno and the page own the timing and
  the transport. Rejected: a shared `run(connect)` function, which would pull
  the server's lifecycle into shared code.
- P-2. The runner imports `browser.ts` from `packages/ridl-transport-ws/e2e/` by
  relative path instead of copying it. It is test code in the same repository.
- P-3. A separate recipe, not a flag on `cabin-ws-demo`, so that `cabin-ws-demo`
  stays in reach of a contributor without a browser.

---

### Task 1: Extract the check into `check.ts`

**Files:**

- Create: `examples/cabin/web/check.ts`
- Create: `examples/cabin/web/check_test.ts`
- Modify: `examples/cabin/web/main.ts` (use `check.ts`; behaviour and output
  unchanged)
- Modify: `justfile` recipe `cabin-ws-demo` (run `deno test check_test.ts` next
  to the existing `deno fmt --check`, `deno lint` and `deno check`)

**Interfaces:**

- Produces, in `check.ts`:
  - `export const LOWEST = -40`, `HIGHEST = 85`, `RING = 126`, `MAX_STEP = 63`
    (with the comments that justify them, moved from `main.ts`).
  - `export interface Progress { received: number; wraps: number;
    sawHighest: boolean; sawLowestAfter: boolean }`
  - `export type Verdict = { kind: "continue" } | { kind: "pass" } |
    { kind: "fail"; cause: string }`
  - `export class TemperatureCheck { accept(value: number): Verdict;
    readonly progress: Progress }`
    — the logic of `onPublish` in `main.ts` today; after a `fail` every later
    call returns the same `fail`.
  - `export function decodeTemperature(payload: Uint8Array): number`
  - `export const TEMPERATURE = { iface: 1, ord: 1 }`

- [ ] **Step 1: Write `check_test.ts`** with these tests (`node:assert/strict`):
  - `a full lap passes`: feed -40..85 then -40; the last call returns `pass`,
    every earlier one `continue`; `wraps === 1`.
  - `a value outside the range fails`: 86 → `fail` naming
    `temperature 86 is outside [-40..85]`.
  - `a step of 64 fails`: -40 then 24 → `fail`; -40 then 23 → `continue`.
  - `a repeat is ignored`: 0, 0, 1 → three `continue`, `received === 3`.
  - `85 as the first sample does not count`: 85, -40 → `continue` twice.
  - `a fail is sticky`: after a `fail`, accepting 0 returns the same cause.
  - `decodeTemperature reads a TemperatureBox`: build a `TemperatureBox` with
    the `flatc` builder for -40 and 85 and decode both.
- [ ] **Step 2:** Run `deno test check_test.ts` in `examples/cabin/web` (after
      `just cabin-ws-demo` once, so `generated/` exists). Expected: FAIL, module
      not found.
- [ ] **Step 3:** Write `check.ts`; change `main.ts` to call
      `check.accept(decodeTemperature(payload))` for `TEMPERATURE` and act on
      the verdict. The final `console.log` line keeps its text, read from
      `progress`.
- [ ] **Step 4:** `deno test check_test.ts` passes; `just cabin-ws-demo` passes.
- [ ] **Step 5:** Mutate: change `MAX_STEP` to 64 and the first-sample rule;
      each must make a test fail. Restore.
- [ ] **Step 6: Commit**
      `refactor(repo): move the cabin demo's check into a
  module the browser can load`

### Task 2: The browser page, its runner and the recipe

**Files:**

- Create: `examples/cabin/web/page.ts` — reads `port` and `hash` (hex) from the
  query string, connects with `encoding: "flatbuffers"` and catalog `veh.cabin`,
  subscribes to `TEMPERATURE`, feeds `TemperatureCheck`, sets
  `globalThis.result` to a promise that resolves with `progress` on `pass` and
  rejects with the cause on `fail` or on a closed connection.
- Create: `examples/cabin/web/browser.ts` — usage
  `deno run -A browser.ts <server-executable>`. Starts the server and reads its
  `port`/`hash` lines (move that reader from `main.ts` into a shared `server.ts`
  with
  `startServer(path): Promise<{ port: number; hash: string;
  stop(): Promise<Deno.CommandStatus> }>`),
  bundles `page.ts` (`deno bundle --platform=browser`, as `e2e/browser_test.ts`
  does), serves `/` and `/page.js` from `Deno.serve` on port 0, `launch()`es the
  browser, `evaluate(url, "globalThis.result")` under a 30 000 ms limit, then
  stops the server and checks its status is 0. Prints
  `cabin-browser: received … sample(s), … wrap(s)` and exits 0, or names the
  cause and exits 1.
- Create: `examples/cabin/web/server.ts` (above); `main.ts` uses it.
- Modify: `justfile` — new recipe `cabin-browser-demo: _cabin-generate` doing
  the `flatc`/`deno` checks and generation of `cabin-ws-demo` (factor them into
  a private recipe `_cabin-web-generate` that both call), the browser lookup of
  `ts-browser-e2e`, then `deno run -A browser.ts "$server"`. Comment states it
  is not a member of `build` and why.
- Modify: `.github/workflows/ci.yml` — in the job that runs
  `just ts-browser-e2e`, install `flatc` the way the `cabin-ws-demo` job does
  and add `run: just cabin-browser-demo`.
- Modify: `AGENTS.md` and `CONTRIBUTING.md` command lists — add the recipe next
  to `ts-browser-e2e`.

**Interfaces:**

- Consumes: Task 1's `TemperatureCheck`, `decodeTemperature`, `TEMPERATURE`;
  `launch(): Promise<Browser>` from `packages/ridl-transport-ws/e2e/browser.ts`.

- [ ] **Step 1:** Write `page.ts`, `server.ts`, `browser.ts`; switch `main.ts`
      to `server.ts`.
- [ ] **Step 2:** `just cabin-browser-demo` passes locally (Chrome on macOS);
      `just cabin-ws-demo` still passes.
- [ ] **Step 3: Failure paths.** Each must exit non-zero within the deadline and
      name the cause; restore after each:
  - edit `page.ts` to `throw` before connecting → the page's exception;
  - make `decodeTemperature` add 1 → the range or the bound check fails;
  - pass a non-existent server path → names the server;
  - `RIDL_BROWSER=/nonexistent` → the recipe's browser message. After each run,
    `pgrep -f server` and `pgrep -f headless` show no leftover.
- [ ] **Step 4:** `deno fmt --check`, `deno lint`, `deno check` over
      `examples/cabin/web`; `just gate-parity`, `just link-check`,
      `just story-id-check`.
- [ ] **Step 5: Commit**
      `feat(repo): run the cabin temperature check in a
  headless browser`
