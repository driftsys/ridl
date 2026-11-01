import assert from "node:assert/strict";
import {
  connect,
  decode,
  FRAME_VERSION_3,
  SUBPROTOCOL,
  SUBPROTOCOL_V1,
  SUBPROTOCOL_V3,
} from "../mod.ts";
import { FakeSocket } from "./fake_socket.ts";

Deno.test("the package version is the workspace version", async () => {
  const cargo = await Deno.readTextFile(
    new URL("../../../Cargo.toml", import.meta.url),
  );
  const afterHeader = cargo.split(/^\[workspace\.package\]\s*$/m)[1] ?? "";
  // The section ends at the next line that starts with `[`.
  const section = afterHeader.split(/^\[/m)[0];
  const match = section.match(/^version\s*=\s*"([^"]+)"/m);
  assert.ok(match, "no version under [workspace.package]");
  const config = JSON.parse(
    await Deno.readTextFile(new URL("../deno.json", import.meta.url)),
  );
  assert.equal(config.version, match[1]);
});

Deno.test("ridl-ws-v3_is_offered_first_and_selected", async () => {
  const sockets: FakeSocket[] = [];
  const pending = connect("ws://127.0.0.1:9000", {
    catalog: { name: "veh.cabin", hash: new Uint8Array(32) },
    webSocket: (url, protocols) => {
      const socket = new FakeSocket(url, protocols);
      sockets.push(socket);
      return socket as unknown as WebSocket;
    },
  });
  const socket = sockets[0];
  assert.deepEqual(socket.protocols, [
    SUBPROTOCOL_V3,
    SUBPROTOCOL,
    SUBPROTOCOL_V1,
  ]);
  socket.open(SUBPROTOCOL_V3);
  const sent = decode(socket.sent[0]);
  assert.ok(sent.ok && sent.frame.form === "attach");
  assert.equal(sent.frame.frameVersion, FRAME_VERSION_3);
  socket.deliver({ form: "attached", outcome: { outcome: "accepted" } });
  const session = await pending;
  session.close();
  socket.serverClose(1000);
  await session.closed;
});
