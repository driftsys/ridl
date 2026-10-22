# Cabin WebSocket server and Deno demo — plan

Step 2 of the plan change in
[the TypeScript wasm codec design](../wip/2026-10-21-ts-wasm-codec-design.md): a
Deno program reads cabin's `temperature` signal from a Rust server over a
WebSocket session with the encoding `flatbuffers`, and decodes the payload with
code that `flatc --ts` generates from the `.fbs` that
`ridl build --emit flatbuffers` writes.

## Decisions (delegated, to report to Sebastien)

- **P-1.** The server is a new member of the `examples/cabin` cargo workspace,
  `examples/cabin/server`, a binary. It links the generated crate,
  `ridl-loopback` and `ridl-transport-ws` by path. It follows the shape of
  `crates/ridl-transport-ws/examples/loopback_server.rs`: bind `127.0.0.1:0`,
  print `port <n>`, serve each connection on its own thread over a new
  attachment, exit with status 0 when standard input reaches its end.
- **P-2.** The server publishes `temperature` through the generated `Publisher`
  every 10 ms, cycling through values in `[-40..85]`, so a reader sees a value
  change.
- **P-3.** The server prints a second line `hash <64 hex digits>`, the catalog
  hash of the generated crate, and the Deno program passes it to `attach`. A
  generated TypeScript constant replaces this in step 5.
- **P-4.** The (interface, ordinal) to type map is hand-written in the Deno
  program, as the design allows for this step.
- **P-5.** The Deno program lives at `examples/cabin/web/`. It imports
  `packages/ridl-transport-ws` by relative path and the npm `flatbuffers`
  package at an exact version. The `flatc` output and the `.fbs` are build
  outputs written by the recipe, not committed, the same as `generated/`.
- **P-6.** A new recipe `cabin-ws-demo` runs the whole proof: build the CLI,
  generate the crate and the `.fbs`, run `flatc --ts --gen-object-api`, build
  the server, run the Deno program against it. The program fails unless it
  receives at least two distinct temperature values, each in `[-40..85]`. The
  recipe joins `build`, the `expected` list of `gate-parity`, and CI, which
  installs `flatc` at a pinned version (25.12.19, the version the spike used).

## Tasks

1. **Server.** `examples/cabin/server`, its `Cargo.toml` entry in the workspace,
   the updated `examples/cabin/Cargo.lock`. Done when the server runs, prints
   both lines, and a WebSocket attach with the printed hash and encoding
   `flatbuffers` succeeds.
2. **Deno demo and recipe.** `examples/cabin/web/`, the `cabin-ws-demo` recipe,
   `gate-parity`, CI with `flatc`, and the AGENTS.md command list. Done when
   `just cabin-ws-demo` passes, and fails when the server publishes a value the
   program does not expect (checked once by hand, then reverted).
