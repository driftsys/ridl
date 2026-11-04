# Debugging a machine

A debugger reaches a machine of a deployment in one of two ways, and each uses
its own WebSocket connection.

- A **listener** reads what the machine publishes without taking part in it. It
  sees every signal and every event of every catalog the machine offers or maps,
  and it may send commands and queries. It writes no signal and raises no event.
- A **provider** takes the place of one instance N of the machine. From a
  browser or a test program it publishes N's signals, raises N's events and
  answers the calls that the other components of the machine send to N.

This chapter shows how to run the example debug gateway of `ridl-transport-ws`,
how to listen through it and how to inject through it from the TypeScript
client.

## What is built

The gateway is the example `debug_gateway` of the `ridl-transport-ws` crate. It
reads a deployment descriptor and the name of one machine, builds the
`ridl-loopback` node of that machine, and serves the node over WebSocket.

- **Listening.** For each connection to its listening port, the gateway reads
  the catalog name and hash in the client's `attach`, finds a matching catalog
  among the node's catalogs, and attaches to it as the listener. A client
  therefore attaches once per catalog, as the binding requires. It can subscribe
  to any signal and event of that catalog and send a command or a query. A
  client that publishes is closed with code 1002.
- **Injection.** When the gateway is started with an injection port, it serves a
  second listener. A connection to it uses the subprotocol
  `ridl-ws-v2-provider`, in which the roles of the frame are reversed: the
  server sends `attach` and subscribes to the signals and events of the
  instance, and the client publishes and answers calls. The gateway attaches to
  the node as the twin of the instance, which is the number −N for the instance
  numbered N. The listening port refuses this subprotocol, and the injection
  port refuses the ordinary one. A gateway started without an injection port
  cannot inject.

Two limits hold today.

- The gateway runs in the same process as the loopback node it serves. A
  component sees the gateway only when it runs in that process and shares that
  node. The gateway does not connect to a runtime in another process.
- The gateway has no access policy beyond binding `127.0.0.1`. An embedder that
  needs one writes its own program over `serve_selected` and
  `serve_provided_selected`
  ([the `ridl-transport-ws` record](../design/ridl-transport-ws.md)).

The record that describes the listener, the twin and the node is
[the deployment descriptor record](../design/deployment-descriptor.md#attaching-to-a-node).

## Run the gateway

Build a deployment descriptor with `ridl build --emit deployment`
([the CLI reference](cli-reference.md)), then run the example:

```text
cargo run -p ridl-transport-ws --features descriptor --example debug_gateway -- \
  <descriptor> <machine> <port> [<inject-port>]
```

`<descriptor>` is the `.deployment.binfb` file, `<machine>` is the name of a
machine of the deployment, and `<port>` is a TCP port; 0 asks the system for any
free port. The optional `<inject-port>` is the port of the injection listener,
with the same meaning. The gateway binds `127.0.0.1` only, so only programs on
the same host reach it. It prints `port <n>` with the listening port and, when
it binds the injection listener, `inject-port <n>`, and then waits for
connections.

## Listen from TypeScript

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
`encoding_unsupported`. The gateway prefers a catalog whose name and hash both
match, including when the machine holds several hashes under one name. A name
the machine does not hold is refused with `unknown_catalog`. When no hash
matches a known name, the gateway selects by name and the session refuses the
incompatible hash with `catalog_mismatch`.

For the complete connection sequence with the generated codec module, see
`examples/cabin/web/main.ts` in the repository.

## Inject from TypeScript

To replace the instance N, stop N first: the gateway refuses the connection
while N is attached to the node, because N and its twin never run at the same
time. Then connect with `provide`, with the path
`/<component>/<instance>/<catalog>` of the URL naming the qualified component,
the instance and the catalog, as the deployment descriptor spells them. The
catalog has to be one that the instance offers. When the machine embeds two
versions of one catalog name, injection selects the first in descriptor order;
the path cannot select the other version, and the client must match that first
version's hash.

```ts
import { provide } from "@driftsys/ridl-transport-ws";

const provision = await provide(
  `ws://127.0.0.1:${injectPort}/veh.cabin.Climate/Main/veh.cabin`,
  {
    catalog: { name: CATALOG_NAME, hash: CATALOG_HASH },
    encoding: "flatbuffers", // required: `provide` has no default
    members: [
      { iface: 1, ord: 1, kind: "signal" },
      { iface: 1, ord: 2, kind: "event" },
      { iface: 1, ord: 3, kind: "command" },
    ],
    onNote(note) {
      console.log(note); // Includes an occurrence raised before subscription.
    },
    onRequest(request) {
      // Answer a command or a query that a component sent to N.
      return { outcome: "accepted" };
    },
  },
);
const now = BigInt(Date.now()) * 1000n; // microseconds
provision.publish(1, 1, temperatureBytes, now);
// Raise events when they occur; without a subscription they are noted and dropped.
provision.raise(1, 2, eventBytes, now);
```

The `encoding` is explicit because the gateway serves FlatBuffers, and the
package accepts the gateway's `attach` only when the catalog hash equals the
given hash exactly. `members` names what the instance provides. `publish` and
`invalidate` update the current signal state even before subscription. After its
`answer`, the provider sends that state as the initial value (§5.1). A `raise`
without a subscription is dropped and recorded through `onNote`; it does not
throw (R-DS-10). It still advances the session's event channel sequence counter
(frame §7). After resubscription, the next delivered occurrence can therefore
show a gap, which the Rust consumer records as loss (§5.2). The payload bytes
are encoded by the caller, as for `connect`.

The gateway answers the handshake before it upgrades the connection:

| Answer | Meaning                                                                                                                                 |
| ------ | --------------------------------------------------------------------------------------------------------------------------------------- |
| `404`  | The path names no instance of the machine and catalog that the instance offers, or it has a query, an empty segment or an extra segment |
| `409`  | The instance N is attached to the node, so the twin is refused                                                                          |

A browser does not show the status; it reports a failed connection.

A command or a query that a component sends to N reaches the handler. The
handler returns the outcome: `accepted` for a command, `reply` with the bytes
for a query, or a contract or `busy` outcome. A handler that returns an outcome
the call does not allow, returns nothing or throws sends no response, and the
caller's own bound ends the call (a command is `Undelivered` and a query is
`Timeout`, after the call's bound or after 60 seconds when it has none). The
gateway has at most 64 calls in flight to a provider; further calls wait on the
node's handler.

## What the machine does not receive

Injection works through the ports of the node, and the ports cannot carry
everything that the connection does. This loss is confined to injection and is
the same for every client.

- **The first value.** The `init` value that a provider sends after the
  subscription writes nothing, because the twin has not published. Consumers
  read the last value that the machine held until the first `publish`.
- **The time stamp.** The node stamps each value with its own clock when it
  commits it. The stamp that the client passes only groups values: consecutive
  values with the same stamp are committed together, and it does not reach the
  machine.
- **The cause of an invalid value.** A value that the client declares invalid is
  invalid on the machine. A live value longer than the member's limit is also
  invalid on the machine, and the machine reports it as declared invalid, not as
  detected.
- **An event.** An occurrence is raised with a new envelope and sequence number
  from the node, not the client's. An occurrence longer than the limit of its
  event is not raised. A gap in the client's sequence is not visible on the
  machine.

`Config::default()` records nothing, including in the gateway examples. To
observe these notes in the gateway process, embed the server with
`Config::with_record` and pass a hook that stores or logs each note. They are
not sent to the client.

## A redundant provider set

When several instances offer the same interface, all of them are attached at
once, and the active one is chosen by a rule that the runtime has to provide;
that rule is not built (driftsys/ridl#1196). The twin replaces N only, so the
other instances of the set stay attached and can still write. Before that rule
exists, injecting on a redundant set needs the other instances stopped.

## Limits

- The reversed role carries frame version 2. It has no stream, so a query whose
  reply is a stream is refused locally with `UnknownInteraction`.
- A browser cannot close a connection with the codes the binding names, and it
  does not show the server's pings. Because of that, `provide` closes without a
  code on a violation and has no liveness timer; the server's check is the one
  that runs.
