# Debugging a machine by listening

A listener reads what a machine publishes without taking part in it. It sees
every signal and every event of every catalog the machine offers or maps, and it
may send commands and queries; it writes no signal and raises no event. This
chapter shows how to run the example debug gateway of `ridl-transport-ws` and
how to connect to it from the TypeScript client.

## What is built

The gateway is the example `debug_gateway` of the `ridl-transport-ws` crate. It
reads a deployment descriptor and the name of one machine, builds the
`ridl-loopback` node of that machine, and serves the node over WebSocket. For
each connection it reads the catalog name in the client's `attach`, finds that
catalog among the node's catalogs, and attaches to it as the listener. A client
therefore attaches once per catalog, as the binding requires, and can subscribe
to any signal and event of that catalog and send a command or a query. A client
that publishes is closed with code 1002.

Two limits hold today.

- The gateway runs in the same process as the loopback node it serves. A
  component sees the gateway only when it runs in that process and shares that
  node. The gateway does not connect to a runtime in another process.
- Injection is not built. A client cannot provide a signal value or an event
  through the gateway, and the gateway has no second listener for it.

Listening is only one half of the debug socket. The record that describes the
listener and the node is
[the deployment descriptor record](../design/deployment-descriptor.md#attaching-to-a-node).

## Run the gateway

Build a deployment descriptor with `ridl build --emit deployment`
([the CLI reference](cli-reference.md)), then run the example:

```text
cargo run -p ridl-transport-ws --features descriptor --example debug_gateway -- \
  <descriptor> <machine> <port>
```

`<descriptor>` is the `.deployment.binfb` file, `<machine>` is the name of a
machine of the deployment, and `<port>` is a TCP port; 0 asks the system for any
free port. The gateway binds `127.0.0.1` only, so only programs on the same host
reach it. It prints one line, `port <n>`, with the port it bound, and then waits
for connections. It has no other access policy; an embedder that needs one
writes its own program over `serve_selected`
([the `ridl-transport-ws` record](../design/ridl-transport-ws.md)).

## Connect from TypeScript

The client of `@driftsys/ridl-transport-ws` asks for proto3 when `connect` is
given no encoding. The gateway serves FlatBuffers because the loopback
attachment encodes FlatBuffers, so the client names it:

```ts
import { connect, Kind } from "@driftsys/ridl-transport-ws";

const session = await connect(`ws://127.0.0.1:${port}`, {
  catalog: { name: CATALOG_NAME, hash: CATALOG_HASH },
  encoding: "flatbuffers",
  interfaces: [Cabin], // the generated tables of the catalog's interfaces
  onPublish(target, sample) {
    // Called for each signal value the listener receives.
  },
});
await session.subscribe({
  kind: Kind.Signal,
  iface: Cabin.temperature.iface,
  ord: Cabin.temperature.ord,
});
```

`CATALOG_NAME` and `CATALOG_HASH` come from the generated codec module of the
catalog. Without `encoding: "flatbuffers"` the gateway refuses the `attach` with
`encoding_unsupported`. The gateway selects by catalog name only. A name the
machine does not hold is refused with `unknown_catalog`; for a known name, the
session checks the hash and refuses an incompatible hash with
`catalog_mismatch`.

For the complete connection sequence with the generated codec module, see
`examples/cabin/web/main.ts` in the repository.
