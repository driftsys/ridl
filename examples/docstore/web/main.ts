// Runs the generated typed stream functions against the Rust document provider.
// Usage: deno run -A main.ts <docstore-server-executable>
import assert from "node:assert/strict";
import { loadCodec } from "../../../packages/ridl-runtime/mod.ts";
import {
  connect,
  type Session,
  SessionError,
} from "../../../packages/ridl-transport-ws/mod.ts";
import { checkBytes, checkChunks, checkDuplex } from "./check.ts";
import {
  CATALOG_HASH,
  CATALOG_NAME,
  DocStore,
  DocStore_chunks,
  DocStore_exchange,
  DocStore_fetch,
  DocStore_put,
  PAYLOAD_TYPES,
  RULES,
} from "./generated/docstore.api.codec.ts";
import type {
  ChunkIndex,
  DocBlock,
  DocChunk,
  DocId,
  DocSize,
} from "./generated/docstore.api.ts";
import { type DocstoreServer, startServer } from "./server.ts";

const DEADLINE_MS = 30_000;
let stage = "starting";
let server: DocstoreServer | undefined;
let session: Session | undefined;
let killServer: (() => void) | undefined;
let stopping = false;
function fail(error: unknown): never {
  console.error(`docstore-web: ${String(error)} (stage: ${stage})`);
  killServer?.();
  Deno.exit(1);
}
const deadline = setTimeout(
  () => fail("overall deadline expired"),
  DEADLINE_MS,
);

try {
  const path = Deno.args[0];
  assert.ok(path, "usage: main.ts <docstore-server-executable>");
  server = await startServer(path, (kill) => (killServer = kill));
  void server.status.then((status) => {
    if (!stopping) fail(`server exited early with status ${status.code}`);
  });
  assert.equal(
    server.hash,
    Array.from(CATALOG_HASH, (byte) => byte.toString(16).padStart(2, "0")).join(
      "",
    ),
    "server and generated catalog hashes must match",
  );
  stage = "loading wasm";
  const codec = await loadCodec(
    await Deno.readFile(
      new URL("./generated/docstore_api.wasm", import.meta.url),
    ),
    { catalogHash: CATALOG_HASH, payloadTypes: PAYLOAD_TYPES, ruleText: RULES },
  );
  stage = "attaching";
  session = await connect(`ws://127.0.0.1:${server.port}`, {
    catalog: { name: CATALOG_NAME, hash: CATALOG_HASH },
    encoding: "flatbuffers",
    interfaces: [DocStore],
  });
  void session.closed.then(() => {
    if (!stopping) fail("session closed before checks completed");
  });
  const context = { transport: session, codec };
  const id = "manual" as DocId;
  const bytes = Uint8Array.from({ length: 204_800 }, (_, i) => i % 251);
  const chunk = (index: number): DocChunk => ({
    index: index as ChunkIndex,
    data: Uint8Array.from(
      { length: 3200 },
      (_, i) => (index * 3200 + i) % 251,
    ) as DocBlock,
  });
  const opts = { bound: 1000 };
  stage = "uploading bytes";
  const { upload, accepted } = DocStore_put(context, {
    name: id,
    size: 204_800n as DocSize,
  }, opts);
  await accepted;
  const writer = upload.getWriter();
  try {
    await writer.write(bytes);
    await writer.close();
  } finally {
    writer.releaseLock();
  }
  // Closing the writer only confirms transmission. The provider keeps these
  // reads pending until it consumes the upload's explicit end.
  stage = "checking committed bytes and named chunks";
  await Promise.all([
    checkBytes(DocStore_fetch(context, id, opts)),
    checkChunks(DocStore_chunks(context, id, opts)),
  ]);

  stage = "checking duplex exchange";
  await checkDuplex(DocStore_exchange(context, id, opts), chunk);

  for (const action of ["cancel", "abort"] as const) {
    stage = `checking duplex ${action}`;
    const exchange = DocStore_exchange(context, id, opts);
    const writer = exchange.upload.getWriter();
    const reader = exchange.stream.getReader();
    writer.closed.catch(() => {});
    reader.closed.catch(() => {});
    try {
      await writer.write(chunk(0));
      assert.deepEqual(await reader.read(), { done: false, value: chunk(0) });
      const interrupted = assert.rejects(
        action === "cancel" ? writer.closed : reader.closed,
        (error: unknown) => {
          assert.ok(error instanceof SessionError);
          assert.equal(error.failure.category, "interrupted");
          return true;
        },
      );
      if (action === "cancel") await reader.cancel();
      else await writer.abort();
      await interrupted;
    } finally {
      writer.releaseLock();
      reader.releaseLock();
    }
    // Reuse the same provider member, so locally rejected promises alone
    // cannot hide an exchange still occupying the remote member's slot.
    await checkDuplex(DocStore_exchange(context, id, opts), chunk);
  }
  stage = "stopping";
  stopping = true;
  session.close();
  await session.closed;
  assert.equal((await server.stop()).code, 0, "server exits successfully");
  clearTimeout(deadline);
  console.log(
    "docstore-web: 204800 exact bytes, 64 named chunks, 65 duplex echoes, cancel and abort passed",
  );
} catch (error) {
  fail(error);
}
